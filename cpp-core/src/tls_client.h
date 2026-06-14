#pragma once

#include <openssl/ssl.h>
#include <openssl/x509.h>

#include <memory>
#include <string>
#include <vector>

namespace gci {

struct ParsedUrl {
    std::string scheme;   // ожидается "https"
    std::string host;     // hostname без порта
    uint16_t    port{443};
    std::string path;     // включая query, начиная с "/"
};

bool parse_url(const std::string& url, ParsedUrl& out, std::string& error);

struct TlsConnection {
    // BIO владеет SSL: освобождение SSL произойдёт через BIO_free_all.
    // Не храним отдельный unique_ptr<SSL>, чтобы исключить double-free.
    using SslCtxPtr = std::unique_ptr<SSL_CTX, decltype(&SSL_CTX_free)>;
    using BioPtr    = std::unique_ptr<BIO,     decltype(&BIO_free_all)>;

    SslCtxPtr ctx{nullptr, SSL_CTX_free};
    BioPtr    bio{nullptr, BIO_free_all};

    std::string negotiated_version;   // "TLS 1.2" | "TLS 1.3"
    std::string negotiated_cipher;
    long        verify_result{X509_V_OK};

    /** Возвращает SSL*, принадлежащий BIO. Не освобождать вручную. */
    SSL* ssl() const noexcept;
};

/**
 * TlsClient инкапсулирует создание SSL_CTX, загрузку trust store,
 * выполнение TLS handshake к host:port и предоставление готового SSL*.
 *
 * Hostname verification и проверка цепочки выполняются вызывающим кодом
 * (inspector) уже после успешного connect, чтобы получить детальные
 * флаги для DTO даже при частичной валидации.
 */
class TlsClient {
public:
    TlsClient(const std::string& trust_store_path,
              int timeout_ms = 15000);

    /**
     * Выполняет TCP+TLS handshake.
     * Возвращает true при успешном handshake; verify-результат не блокирует
     * соединение (мы хотим показать пользователю детали даже невалидной цепочки).
     */
    bool connect(const ParsedUrl& url,
                 TlsConnection& out,
                 std::string& error);

private:
    std::string trust_store_path_;
    int         timeout_ms_;

    bool load_trust_store(SSL_CTX* ctx, std::string& error);
};

}  // namespace gci
