#include "tls_client.h"

#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>
#include <openssl/provider.h>

#ifdef GCI_GOST_ENABLED
#include "gost_provider_init.h"
#endif

#include <cctype>
#include <cstring>
#include <filesystem>
#include <mutex>
#include <sstream>

namespace gci {

// ── GOST provider status (для inspector.cpp) ──
bool gost_provider_loaded() {
#ifdef GCI_GOST_ENABLED
    return OSSL_PROVIDER_available(nullptr, "gost") == 1;
#else
    return false;
#endif
}

namespace {

// ── GOST provider initialization (один раз) ──
void ensure_gost_provider() {
#ifdef GCI_GOST_ENABLED
    static bool initialized = false;
    static std::mutex init_mutex;
    std::lock_guard<std::mutex> lock(init_mutex);
    if (initialized) return;
    initialized = true;

    // 1. Default provider — содержит стандартные AES/RSA/SHA алгоритмы.
    //    Без него ничего не работает в OpenSSL 3.x.
    OSSL_PROVIDER* default_prov = OSSL_PROVIDER_load(nullptr, "default");
    if (!default_prov) {
        // Критично, но не фатально — try to continue
        ERR_clear_error();
    }

    // 2. Legacy provider — нужен для некоторых старых алгоритмов которые
    //    использует gost-engine (MD5, RIPEMD160 и др.)
    OSSL_PROVIDER_load(nullptr, "legacy");
    ERR_clear_error();

    // 3. Наш GOST provider, статически слинкованный.
    //    Имя символа init-функции переименовано из OSSL_provider_init
    //    в gost_provider_init на этапе сборки (см. build-gost-engine-android.sh).
    if (OSSL_PROVIDER_add_builtin(nullptr, "gost", gost_provider_init) != 1) {
        ERR_clear_error();
        return;
    }

    OSSL_PROVIDER* gost_prov = OSSL_PROVIDER_load(nullptr, "gost");
    if (!gost_prov) {
        ERR_clear_error();
        return;
    }
    // gost_prov загружен — теперь будут доступны ГОСТ-алгоритмы и cipher suites.
#endif
}

std::string openssl_last_error() {
    BIO* bio = BIO_new(BIO_s_mem());
    ERR_print_errors(bio);
    char* buf = nullptr;
    long len = BIO_get_mem_data(bio, &buf);
    std::string out(buf ? buf : "", len > 0 ? static_cast<size_t>(len) : 0);
    BIO_free(bio);
    if (out.empty()) out = "unknown OpenSSL error";
    return out;
}

std::string tls_version_name(const SSL* ssl) {
    if (!ssl) return {};
    const int v = SSL_version(ssl);
    switch (v) {
        case TLS1_2_VERSION: return "TLS 1.2";
        case TLS1_3_VERSION: return "TLS 1.3";
        default:             return "unknown";
    }
}

}  // namespace

bool parse_url(const std::string& url, ParsedUrl& out, std::string& error) {
    // Минимальный, но достаточно строгий парсер: scheme://host[:port][/path]
    const std::string scheme_sep = "://";
    auto pos = url.find(scheme_sep);
    if (pos == std::string::npos) {
        error = "URL: missing scheme";
        return false;
    }
    out.scheme = url.substr(0, pos);
    for (auto& c : out.scheme) c = static_cast<char>(std::tolower(c));
    if (out.scheme != "https") {
        error = "URL: only https is supported";
        return false;
    }

    const auto rest_start = pos + scheme_sep.size();
    const auto path_start = url.find('/', rest_start);
    const std::string authority = (path_start == std::string::npos)
        ? url.substr(rest_start)
        : url.substr(rest_start, path_start - rest_start);

    out.path = (path_start == std::string::npos) ? "/" : url.substr(path_start);

    const auto colon = authority.find(':');
    if (colon == std::string::npos) {
        out.host = authority;
        out.port = 443;
    } else {
        out.host = authority.substr(0, colon);
        try {
            const int p = std::stoi(authority.substr(colon + 1));
            if (p <= 0 || p > 65535) throw std::out_of_range("port");
            out.port = static_cast<uint16_t>(p);
        } catch (...) {
            error = "URL: invalid port";
            return false;
        }
    }
    if (out.host.empty()) {
        error = "URL: empty host";
        return false;
    }
    return true;
}

TlsClient::TlsClient(const std::string& trust_store_path, int timeout_ms)
    : trust_store_path_(trust_store_path), timeout_ms_(timeout_ms) {}

bool TlsClient::load_trust_store(SSL_CTX* ctx, std::string& error) {
    if (trust_store_path_.empty()) {
        error = "trust store path is empty";
        return false;
    }

    // ── 1. Загружаем системные CA (чтобы работала верификация любых цепочек) ──
    // SSL_CTX_set_default_verify_paths использует compile-time paths OpenSSL.
    // На desktop (Linux/macOS) это обычно /etc/ssl/certs или Keychain.
    // На Android не работает (нет стандартного пути), поэтому пробуем
    // известные директории с системными CA-сертификатами.
    SSL_CTX_set_default_verify_paths(ctx);

    // Android-specific: попытка загрузить системные CA из известных путей.
    static const char* android_ca_dirs[] = {
        "/system/etc/security/cacerts",
        "/apex/com.android.conscrypt/cacerts",
        "/etc/security/cacerts",
        nullptr
    };
    for (const char** dir = android_ca_dirs; *dir; ++dir) {
        namespace fs = std::filesystem;
        std::error_code ec;
        if (fs::is_directory(*dir, ec)) {
            SSL_CTX_load_verify_dir(ctx, *dir);
            break;
        }
    }

    // ── 2. Загружаем наш кастомный trust-store (Минцифры) поверх системных ──
    namespace fs = std::filesystem;
    std::error_code ec;
    if (!fs::exists(trust_store_path_, ec)) {
        // Не фатально — системные CA уже загружены, просто Mincifry-проверка не будет 100%.
        return true;
    }

    const bool is_dir = fs::is_directory(trust_store_path_, ec);
    if (is_dir) {
        for (auto it = fs::recursive_directory_iterator(trust_store_path_, ec);
             !ec && it != fs::recursive_directory_iterator(); ++it) {
            if (!it->is_regular_file()) continue;
            const auto p = it->path();
            const auto ext = p.extension().string();
            if (ext != ".pem" && ext != ".crt" && ext != ".cer") continue;
            SSL_CTX_load_verify_locations(ctx, p.string().c_str(), nullptr);
        }
    } else {
        SSL_CTX_load_verify_locations(ctx, trust_store_path_.c_str(), nullptr);
    }
    return true;
}

bool TlsClient::connect(const ParsedUrl& url, TlsConnection& out, std::string& error) {
    ensure_gost_provider();

    out.ctx.reset(SSL_CTX_new(TLS_client_method()));
    if (!out.ctx) {
        error = "SSL_CTX_new failed: " + openssl_last_error();
        return false;
    }

    SSL_CTX_set_min_proto_version(out.ctx.get(), TLS1_2_VERSION);
    SSL_CTX_set_max_proto_version(out.ctx.get(), TLS1_3_VERSION);

    // Отключаем заведомо слабые шифры и старые опции
    SSL_CTX_set_options(out.ctx.get(),
        SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 |
        SSL_OP_NO_COMPRESSION |
        SSL_OP_CIPHER_SERVER_PREFERENCE);

    // Разрешаем широкий набор шифров для максимальной совместимости с
    // гос-сайтами. Некоторые российские серверы поддерживают только RSA
    // key exchange (без ECDHE). ГОСТ-шифры добавлены на случай наличия
    // gost-engine — если провайдер не загружен, OpenSSL их просто пропустит.
    //
    // ВАЖНО: SSL_CTX_set_cipher_list возвращает 0 если ВСЕ шифры невалидны,
    // но не ломается если часть строк нераспознана (ГОСТ без провайдера).
    if (SSL_CTX_set_cipher_list(out.ctx.get(),
            "HIGH:MEDIUM"
            ":GOST2012-GOST8912-GOST8912:GOST2001-GOST89-GOST89"
            ":!aNULL:!eNULL:!MD5:!RC4:!3DES:!EXPORT:!DES:!PSK:!SRP") != 1) {
        // Fallback без ГОСТ
        SSL_CTX_set_cipher_list(out.ctx.get(), "HIGH:MEDIUM:!aNULL:!eNULL:!MD5:!RC4:!3DES:!EXPORT");
    }

    // TLS 1.3 cipher suites. ГОСТ TLS 1.3 шифры добавляем через отдельный
    // вызов, чтобы ошибка не сломала стандартные. Если set_ciphersuites
    // вернёт 0 (нераспознанные ГОСТ-имена), пробуем только стандартные.
    if (SSL_CTX_set_ciphersuites(out.ctx.get(),
            "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256"
            ":TLS_GOSTR341112_256_WITH_KUZNYECHIK_CTR_OMAC"
            ":TLS_GOSTR341112_256_WITH_MAGMA_CTR_OMAC"
            ":TLS_GOSTR341112_256_WITH_KUZNYECHIK_MGM_L"
            ":TLS_GOSTR341112_256_WITH_MAGMA_MGM_L"
            ":TLS_GOSTR341112_256_WITH_KUZNYECHIK_MGM_S"
            ":TLS_GOSTR341112_256_WITH_MAGMA_MGM_S") != 1) {
        // ГОСТ TLS 1.3 не поддерживается — ставим только стандартные
        SSL_CTX_set_ciphersuites(out.ctx.get(),
            "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256");
    }

    // Просим OpenSSL валидировать пир, но решение принимаем сами — нам нужны
    // и chain_ok=false случаи с детальной диагностикой.
    // Callback всегда возвращает 1 (OK) — handshake НЕ прерывается при ошибке
    // верификации. Реальный результат проверки читается через
    // SSL_get_verify_result() после хендшейка и передаётся в JSON-ответ.
    SSL_CTX_set_verify(out.ctx.get(), SSL_VERIFY_PEER,
        [](int /*preverify_ok*/, X509_STORE_CTX* /*ctx*/) -> int { return 1; });

    if (!load_trust_store(out.ctx.get(), error)) {
        return false;
    }

    // BIO с автоматическим подключением "host:port"
    std::ostringstream hp;
    hp << url.host << ":" << url.port;
    out.bio.reset(BIO_new_ssl_connect(out.ctx.get()));
    if (!out.bio) {
        error = "BIO_new_ssl_connect failed: " + openssl_last_error();
        return false;
    }
    BIO_set_conn_hostname(out.bio.get(), hp.str().c_str());

    SSL* raw_ssl = nullptr;
    BIO_get_ssl(out.bio.get(), &raw_ssl);
    if (!raw_ssl) {
        error = "BIO_get_ssl failed";
        return false;
    }
    SSL_set_mode(raw_ssl, SSL_MODE_AUTO_RETRY);

    // SNI и hostname для X509_check_host через встроенный verify
    if (SSL_set_tlsext_host_name(raw_ssl, url.host.c_str()) != 1) {
        error = "SNI set failed: " + openssl_last_error();
        return false;
    }
    SSL_set1_host(raw_ssl, url.host.c_str());

    if (BIO_do_connect(out.bio.get()) <= 0) {
        error = "TCP connect failed: " + openssl_last_error();
        return false;
    }
    if (BIO_do_handshake(out.bio.get()) <= 0) {
        error = "TLS handshake failed: " + openssl_last_error();
        return false;
    }

    out.negotiated_version = tls_version_name(raw_ssl);
    if (const char* c = SSL_get_cipher_name(raw_ssl)) out.negotiated_cipher = c;
    out.verify_result = SSL_get_verify_result(raw_ssl);

    return true;
}

SSL* TlsConnection::ssl() const noexcept {
    if (!bio) return nullptr;
    SSL* s = nullptr;
    BIO_get_ssl(bio.get(), &s);
    return s;
}

bool TlsClient::connect_gost_only(const ParsedUrl& url, TlsConnection& out, std::string& error) {
#ifndef GCI_GOST_ENABLED
    error = "GOST provider not compiled in";
    return false;
#else
    if (!gost_provider_loaded()) {
        error = "GOST provider not loaded";
        return false;
    }

    ensure_gost_provider();

    out.ctx.reset(SSL_CTX_new(TLS_client_method()));
    if (!out.ctx) {
        error = "SSL_CTX_new failed: " + openssl_last_error();
        return false;
    }

    SSL_CTX_set_min_proto_version(out.ctx.get(), TLS1_2_VERSION);
    SSL_CTX_set_max_proto_version(out.ctx.get(), TLS1_3_VERSION);
    SSL_CTX_set_options(out.ctx.get(),
        SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 | SSL_OP_NO_COMPRESSION);

    // ТОЛЬКО ГОСТ-шифры — чтобы сервер выбрал именно ГОСТ-сертификат
    SSL_CTX_set_cipher_list(out.ctx.get(),
        "GOST2012-GOST8912-GOST8912:GOST2001-GOST89-GOST89");
    SSL_CTX_set_ciphersuites(out.ctx.get(),
        "TLS_GOSTR341112_256_WITH_KUZNYECHIK_CTR_OMAC"
        ":TLS_GOSTR341112_256_WITH_MAGMA_CTR_OMAC"
        ":TLS_GOSTR341112_256_WITH_KUZNYECHIK_MGM_L"
        ":TLS_GOSTR341112_256_WITH_MAGMA_MGM_L"
        ":TLS_GOSTR341112_256_WITH_KUZNYECHIK_MGM_S"
        ":TLS_GOSTR341112_256_WITH_MAGMA_MGM_S");

    SSL_CTX_set_verify(out.ctx.get(), SSL_VERIFY_PEER,
        [](int, X509_STORE_CTX*) -> int { return 1; });

    if (!load_trust_store(out.ctx.get(), error)) {
        return false;
    }

    std::ostringstream hp;
    hp << url.host << ":" << url.port;
    out.bio.reset(BIO_new_ssl_connect(out.ctx.get()));
    if (!out.bio) {
        error = "BIO_new_ssl_connect failed: " + openssl_last_error();
        return false;
    }
    BIO_set_conn_hostname(out.bio.get(), hp.str().c_str());

    SSL* raw_ssl = nullptr;
    BIO_get_ssl(out.bio.get(), &raw_ssl);
    if (!raw_ssl) {
        error = "BIO_get_ssl failed";
        return false;
    }
    SSL_set_mode(raw_ssl, SSL_MODE_AUTO_RETRY);

    if (SSL_set_tlsext_host_name(raw_ssl, url.host.c_str()) != 1) {
        error = "SNI set failed: " + openssl_last_error();
        return false;
    }
    SSL_set1_host(raw_ssl, url.host.c_str());

    if (BIO_do_connect(out.bio.get()) <= 0) {
        error = "TCP connect (GOST) failed: " + openssl_last_error();
        return false;
    }
    if (BIO_do_handshake(out.bio.get()) <= 0) {
        error = "TLS handshake (GOST) failed: " + openssl_last_error();
        return false;
    }

    out.negotiated_version = tls_version_name(raw_ssl);
    if (const char* c = SSL_get_cipher_name(raw_ssl)) out.negotiated_cipher = c;
    out.verify_result = SSL_get_verify_result(raw_ssl);
    return true;
#endif
}

}  // namespace gci
