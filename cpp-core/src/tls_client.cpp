#include "tls_client.h"

#include <openssl/err.h>
#include <openssl/ssl.h>
#include <openssl/x509v3.h>

#include <cctype>
#include <cstring>
#include <filesystem>
#include <sstream>

namespace gci {

namespace {

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

    namespace fs = std::filesystem;
    std::error_code ec;
    if (!fs::exists(trust_store_path_, ec)) {
        error = "trust store path does not exist: " + trust_store_path_;
        return false;
    }

    const bool is_dir = fs::is_directory(trust_store_path_, ec);
    if (is_dir) {
        // Загружаем все *.pem из каталога (включая roots/ и intermediates/).
        bool any = false;
        for (auto it = fs::recursive_directory_iterator(trust_store_path_, ec);
             !ec && it != fs::recursive_directory_iterator(); ++it) {
            if (!it->is_regular_file()) continue;
            const auto p = it->path();
            const auto ext = p.extension().string();
            if (ext != ".pem" && ext != ".crt" && ext != ".cer") continue;
            if (SSL_CTX_load_verify_locations(ctx, p.string().c_str(), nullptr) == 1) {
                any = true;
            }
        }
        if (!any) {
            error = "no usable certificates in trust store: " + trust_store_path_;
            return false;
        }
    } else {
        if (SSL_CTX_load_verify_locations(ctx, trust_store_path_.c_str(), nullptr) != 1) {
            error = "failed to load trust file: " + openssl_last_error();
            return false;
        }
    }
    return true;
}

bool TlsClient::connect(const ParsedUrl& url, TlsConnection& out, std::string& error) {
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
    SSL_CTX_set_cipher_list(out.ctx.get(),
        "ECDHE+AESGCM:ECDHE+CHACHA20:DHE+AESGCM:!aNULL:!eNULL:!MD5:!RC4:!3DES");
    SSL_CTX_set_ciphersuites(out.ctx.get(),
        "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256");

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

}  // namespace gci
