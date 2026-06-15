#pragma once

#include <openssl/ssl.h>

#include <cstddef>
#include <string>

namespace gci {

struct HttpResponse {
    int         status_code{0};
    std::string status_line;
    std::string body;
    bool        truncated{false};
};

class HttpClient {
public:
    explicit HttpClient(std::size_t max_body_bytes = 1024 * 1024)
        : max_body_bytes_(max_body_bytes) {}

    bool get(BIO* bio,
             const std::string& host,
             const std::string& path,
             HttpResponse& out,
             std::string& error);

private:
    std::size_t max_body_bytes_;
};

}
