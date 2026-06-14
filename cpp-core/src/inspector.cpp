#include "inspector.h"

#include "http_client.h"
#include "tls_client.h"
#include "x509_parser.h"

#include <nlohmann/json.hpp>

#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/x509.h>

#include <cstring>
#include <ctime>
#include <memory>
#include <mutex>
#include <random>
#include <string>
#include <unordered_set>
#include <vector>

using json = nlohmann::json;

namespace {

void ensure_openssl_init() {
    static std::once_flag flag;
    std::call_once(flag, []() {
        OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS |
                         OPENSSL_INIT_LOAD_CRYPTO_STRINGS, nullptr);
    });
}

std::string make_request_id() {
    static thread_local std::mt19937_64 rng{std::random_device{}()};
    static const char* hex = "0123456789abcdef";
    std::string id(32, '0');
    auto v = rng();
    for (int i = 0; i < 16; ++i) { id[i] = hex[(v >> (i * 4)) & 0xF]; }
    v = rng();
    for (int i = 0; i < 16; ++i) { id[16 + i] = hex[(v >> (i * 4)) & 0xF]; }
    return id;
}

json cert_to_json(const gci::CertInfo& c) {
    return json{
        {"subject", c.subject},
        {"issuer", c.issuer},
        {"serialNumber", c.serial_number},
        {"validFrom", c.valid_from},
        {"validTo", c.valid_to},
        {"san", c.san},
        {"cn", c.cn},
        {"fingerprintSha256", c.fingerprint_sha256},
        {"signatureAlgorithm", c.signature_algorithm}
    };
}

json chain_entry_to_json(const gci::CertInfo& c) {
    return json{
        {"subject", c.subject},
        {"issuer", c.issuer},
        {"serialNumber", c.serial_number},
        {"validFrom", c.valid_from},
        {"validTo", c.valid_to},
        {"fingerprintSha256", c.fingerprint_sha256}
    };
}

/**
 * Множество маркеров, по которым мы признаём, что сертификат выпущен
 * УЦ Минцифры. Список открытый — расширяется при пополнении trust-store
 * (см. trust-store/manifest.json).
 */
const std::unordered_set<std::string>& mincifry_subject_markers() {
    static const std::unordered_set<std::string> set = {
        "russian trusted root ca",
        "russian trusted sub ca",
        "ministry of digital development",
        "минцифры россии",
        "минцифра",
        "минцифры",
        "russian trusted"
    };
    return set;
}

bool name_contains_mincifry(const std::string& name) {
    std::string lower;
    lower.reserve(name.size());
    for (char c : name) lower.push_back(static_cast<char>(std::tolower(static_cast<unsigned char>(c))));
    for (const auto& m : mincifry_subject_markers()) {
        if (lower.find(m) != std::string::npos) return true;
    }
    return false;
}

bool chain_signed_by_mincifry(const std::vector<gci::CertInfo>& chain) {
    // Если в цепочке хотя бы один корневой/промежуточный сертификат
    // содержит маркеры Минцифры — считаем сертификат выданным УЦ Минцифры.
    for (size_t i = 1; i < chain.size(); ++i) {
        if (name_contains_mincifry(chain[i].subject)) return true;
    }
    // Дополнительно: issuer end-entity.
    if (!chain.empty() && name_contains_mincifry(chain[0].issuer)) return true;
    return false;
}

bool not_expired(const gci::CertInfo& c) {
    // Сравнение ISO8601 строк работает лексикографически благодаря фиксированному формату.
    char now_buf[32];
    std::time_t t = std::time(nullptr);
    std::tm tm_utc{};
#ifdef _WIN32
    gmtime_s(&tm_utc, &t);
#else
    gmtime_r(&t, &tm_utc);
#endif
    std::strftime(now_buf, sizeof(now_buf), "%Y-%m-%dT%H:%M:%SZ", &tm_utc);
    const std::string now(now_buf);
    return !c.valid_from.empty() && !c.valid_to.empty()
        && c.valid_from <= now && now <= c.valid_to;
}

json error_obj(const std::string& code, const std::string& message) {
    return json{{"code", code}, {"message", message}};
}

std::string build_error_response(const std::string& request_id,
                                 const std::string& input_url,
                                 const std::string& code,
                                 const std::string& msg) {
    json out = {
        {"requestId", request_id},
        {"inputUrl", input_url},
        {"resolvedHost", ""},
        {"tlsVersion", ""},
        {"tlsCipher", ""},
        {"isGostCipher", false},
        {"gostSupported", gci::gost_provider_loaded()},
        {"certificate", nullptr},
        {"chain", json::array()},
        {"validation", {
            {"hostname_ok", false},
            {"chain_ok", false},
            {"expired_ok", false},
            {"mincifry_ca_ok", false}
        }},
        {"is_mintsifry_ca", false},
        {"html", ""},
        {"errors", json::array({error_obj(code, msg)})}
    };
    return out.dump();
}

char* dup_to_c(const std::string& s) {
    char* buf = static_cast<char*>(std::malloc(s.size() + 1));
    if (!buf) return nullptr;
    std::memcpy(buf, s.data(), s.size());
    buf[s.size()] = '\0';
    return buf;
}

}  // namespace

extern "C" {

const char* inspector_version(void) {
    return "1.0.0";
}

void inspector_free_string(const char* ptr) {
    if (ptr) std::free(const_cast<char*>(ptr));
}

const char* inspect_url(const char* request_json) {
    ensure_openssl_init();

    std::string request_id = make_request_id();
    std::string input_url;
    std::string trust_store_path;
    bool load_html = true;
    int timeout_ms = 15000;
    std::size_t max_html_bytes = 1024 * 1024;

    if (!request_json) {
        return dup_to_c(build_error_response(request_id, "", "BAD_REQUEST", "null request_json"));
    }

    try {
        const json req = json::parse(request_json);
        input_url        = req.value("url", "");
        trust_store_path = req.value("trustStorePath", "");
        load_html        = req.value("loadHtml", true);
        timeout_ms       = req.value("timeoutMs", 15000);
        max_html_bytes   = req.value("maxHtmlBytes", static_cast<std::size_t>(1024 * 1024));
        if (req.contains("requestId") && req["requestId"].is_string()) {
            request_id = req["requestId"].get<std::string>();
        }
    } catch (const std::exception& e) {
        return dup_to_c(build_error_response(request_id, input_url, "BAD_JSON", e.what()));
    }

    if (input_url.empty()) {
        return dup_to_c(build_error_response(request_id, input_url, "EMPTY_URL", "url is required"));
    }
    if (trust_store_path.empty()) {
        return dup_to_c(build_error_response(request_id, input_url, "EMPTY_TRUST", "trustStorePath is required"));
    }

    gci::ParsedUrl parsed;
    std::string err;
    if (!gci::parse_url(input_url, parsed, err)) {
        return dup_to_c(build_error_response(request_id, input_url, "URL_PARSE", err));
    }

    gci::TlsClient client(trust_store_path, timeout_ms);
    gci::TlsConnection conn;
    if (!client.connect(parsed, conn, err)) {
        // ── Распознаём типичные ошибки и даём понятное сообщение ──
        std::string user_msg = err;
        std::string code = "TLS_HANDSHAKE";

        // VPN / прокси обрывает соединение
        if (err.find("unexpected eof") != std::string::npos ||
            err.find("connection reset") != std::string::npos ||
            err.find("broken pipe") != std::string::npos) {
            code = "CONNECTION_RESET";
            user_msg = "Соединение прервано удалённой стороной. "
                       "Если используется VPN или прокси — попробуйте отключить их и повторить.";
        }
        // Таймаут / сеть недоступна
        else if (err.find("timed out") != std::string::npos ||
                 err.find("timeout") != std::string::npos) {
            code = "TIMEOUT";
            user_msg = "Время ожидания подключения истекло. Проверьте интернет-соединение.";
        }
        // DNS
        else if (err.find("resolve") != std::string::npos ||
                 err.find("getaddrinfo") != std::string::npos ||
                 err.find("Name or service not known") != std::string::npos) {
            code = "DNS_FAILED";
            user_msg = "Не удалось разрешить доменное имя. Проверьте интернет-соединение.";
        }
        // Сервер отказал / TCP refused
        else if (err.find("Connection refused") != std::string::npos ||
                 err.find("connect failed") != std::string::npos) {
            code = "CONNECTION_REFUSED";
            user_msg = "Сервер отклонил подключение на порту 443.";
        }
        // ГОСТ TLS — нет общих шифров
        else if (err.find("no shared cipher") != std::string::npos ||
                 err.find("no ciphers available") != std::string::npos ||
                 err.find("no protocols available") != std::string::npos ||
                 err.find("sslv3 alert handshake failure") != std::string::npos ||
                 err.find("handshake failure") != std::string::npos) {
#ifdef GCI_GOST_ENABLED
            code = "TLS_CIPHER_MISMATCH";
            user_msg = "Не удалось согласовать шифр с сервером. "
                       "ГОСТ-поддержка активна, но сервер может требовать "
                       "дополнительную конфигурацию (например, клиентский сертификат).";
#else
            code = "GOST_UNSUPPORTED";
            user_msg = "Сайт использует только ГОСТ-шифрование (российский стандарт). "
                       "В текущей сборке поддержка ГОСТ отсутствует — "
                       "сертификат этого сайта нельзя проверить.";
#endif
        }

        json out = json::parse(build_error_response(request_id, input_url, code, user_msg));
        out["resolvedHost"] = parsed.host;
        return dup_to_c(out.dump());
    }

    // SSL принадлежит BIO. Получаем сырой указатель через accessor.
    SSL* ssl = conn.ssl();

    std::unique_ptr<X509, decltype(&X509_free)> peer(
        SSL_get1_peer_certificate(ssl), X509_free);

    json errors = json::array();

    gci::CertInfo end_entity;
    std::vector<gci::CertInfo> chain;
    bool hostname_ok = false;
    bool chain_ok    = false;
    bool expired_ok  = false;
    bool mincifry_ok = false;

    if (!peer) {
        errors.push_back(error_obj("NO_PEER_CERT", "Server did not present a certificate"));
    } else {
        end_entity   = gci::X509Parser::from_cert(peer.get());
        chain        = gci::X509Parser::chain_from_ssl(ssl);
        hostname_ok  = gci::X509Parser::check_hostname(peer.get(), parsed.host);
        expired_ok   = not_expired(end_entity);
        mincifry_ok  = chain_signed_by_mincifry(chain);

        // ── Определяем chain_ok ──
        // Наша задача — показать, целая ли цепочка, а НЕ проверять доверие к
        // корневому CA. Доверие к CA — забота браузера/ОС, а мы инспектор.
        //
        // Коды OpenSSL 19/20 означают лишь «корневой CA не найден в нашем
        // локальном хранилище». На Android системные CA часто недоступны
        // напрямую из OpenSSL, поэтому для ЛЮБЫХ сайтов (YouTube, Let's
        // Encrypt, Google Trust Services и т.д.) будет код 19 или 20.
        // Это НЕ ошибка цепочки — цепочка может быть полностью валидна.
        //
        // chain_ok = true если:
        //   - OpenSSL полностью верифицировал (V_OK), ИЛИ
        //   - Ошибка лишь в том, что корень не в локальном хранилище (19/20),
        //     при этом сервер прислал хотя бы 2 сертификата (end-entity +
        //     хотя бы 1 промежуточный/корневой).
        if (conn.verify_result == X509_V_OK) {
            chain_ok = true;
        } else if ((conn.verify_result == 19 || conn.verify_result == 20)
                   && chain.size() >= 2) {
            // Цепочка структурно полная, просто корень не в нашем store.
            chain_ok = true;
        }

        if (!chain_ok) {
            std::string chain_msg;
            switch (conn.verify_result) {
                case 10: // X509_V_ERR_CERT_HAS_EXPIRED
                    chain_msg = "Один из сертификатов в цепочке просрочен.";
                    break;
                case 2:  // X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT
                    chain_msg = "Цепочка неполная: сервер не прислал промежуточный сертификат.";
                    break;
                case 21: // X509_V_ERR_UNABLE_TO_VERIFY_LEAF_SIGNATURE
                    chain_msg = "Не удалось проверить подпись конечного сертификата.";
                    break;
                default:
                    chain_msg = "Ошибка проверки цепочки (код " + std::to_string(conn.verify_result) + ")";
                    break;
            }
            errors.push_back(error_obj("CHAIN_INVALID", chain_msg));
        }
        if (!hostname_ok) {
            errors.push_back(error_obj("HOSTNAME_MISMATCH",
                "Certificate is not valid for host " + parsed.host));
        }
        if (!expired_ok) {
            errors.push_back(error_obj("EXPIRED", "Certificate is outside its validity window"));
        }
    }

    std::string html;
    if (load_html) {
        gci::HttpClient http(max_html_bytes);
        gci::HttpResponse resp;
        std::string http_err;
        if (http.get(conn.bio.get(), parsed.host, parsed.path, resp, http_err)) {
            html = resp.body;
            if (resp.truncated) {
                errors.push_back(error_obj("HTML_TRUNCATED",
                    "HTML response truncated at " + std::to_string(max_html_bytes) + " bytes"));
            }
            if (resp.status_code >= 400) {
                errors.push_back(error_obj("HTTP_STATUS",
                    "HTTP " + std::to_string(resp.status_code)));
            }
        } else {
            errors.push_back(error_obj("HTTP_GET", http_err));
        }
    }

    json out = {
        {"requestId", request_id},
        {"inputUrl", input_url},
        {"resolvedHost", parsed.host},
        {"tlsVersion", conn.negotiated_version},
        {"tlsCipher", conn.negotiated_cipher},
        {"isGostCipher", conn.negotiated_cipher.find("GOST") != std::string::npos ||
                         conn.negotiated_cipher.find("KUZNYECHIK") != std::string::npos ||
                         conn.negotiated_cipher.find("MAGMA") != std::string::npos},
        {"gostSupported", gci::gost_provider_loaded()},
        {"certificate", peer ? cert_to_json(end_entity) : json(nullptr)},
        {"chain", json::array()},
        {"validation", {
            {"hostname_ok", hostname_ok},
            {"chain_ok", chain_ok},
            {"expired_ok", expired_ok},
            {"mincifry_ca_ok", mincifry_ok}
        }},
        {"is_mintsifry_ca", mincifry_ok},
        {"html", html},
        {"errors", errors}
    };
    for (const auto& c : chain) out["chain"].push_back(chain_entry_to_json(c));

    return dup_to_c(out.dump());
}

}  // extern "C"
