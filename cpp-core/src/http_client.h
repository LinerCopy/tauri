#pragma once

#include <openssl/ssl.h>

#include <cstddef>
#include <string>

namespace gci {

struct HttpResponse {
    int         status_code{0};
    std::string status_line;
    std::string body;          // ограничено max_body_bytes
    bool        truncated{false};
};

class HttpClient {
public:
    explicit HttpClient(std::size_t max_body_bytes = 1024 * 1024)
        : max_body_bytes_(max_body_bytes) {}

    /**
     * Выполняет HTTP/1.1 GET через уже установленное TLS-соединение (BIO).
     * Заголовок Host обязателен. body читается до Content-Length или до
     * закрытия соединения. Размер ответа ограничивается max_body_bytes.
     */
    bool get(BIO* bio,
             const std::string& host,
             const std::string& path,
             HttpResponse& out,
             std::string& error);

private:
    std::size_t max_body_bytes_;
};

}  // namespace gci
