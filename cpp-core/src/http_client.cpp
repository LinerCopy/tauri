#include "http_client.h"

#include <openssl/bio.h>

#include <algorithm>
#include <cctype>
#include <cstring>
#include <sstream>

namespace gci {

namespace {

std::string to_lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return s;
}

bool write_all(BIO* bio, const std::string& data, std::string& error) {
    size_t total = 0;
    while (total < data.size()) {
        const int rc = BIO_write(bio, data.data() + total, static_cast<int>(data.size() - total));
        if (rc <= 0) {
            if (BIO_should_retry(bio)) continue;
            error = "BIO_write failed";
            return false;
        }
        total += static_cast<size_t>(rc);
    }
    return true;
}

}  // namespace

bool HttpClient::get(BIO* bio,
                     const std::string& host,
                     const std::string& path,
                     HttpResponse& out,
                     std::string& error) {
    if (!bio) {
        error = "null BIO";
        return false;
    }

    std::ostringstream req;
    req << "GET " << (path.empty() ? "/" : path) << " HTTP/1.1\r\n"
        << "Host: " << host << "\r\n"
        << "User-Agent: GosCertInspector/1.0 (+local-only)\r\n"
        << "Accept: text/html,application/xhtml+xml;q=0.9,*/*;q=0.5\r\n"
        << "Accept-Encoding: identity\r\n"
        << "Connection: close\r\n\r\n";

    if (!write_all(bio, req.str(), error)) return false;

    std::string raw;
    raw.reserve(8192);
    char buf[4096];

    // Сначала читаем достаточно, чтобы найти конец заголовков "\r\n\r\n".
    size_t header_end = std::string::npos;
    while (header_end == std::string::npos) {
        const int rc = BIO_read(bio, buf, sizeof(buf));
        if (rc > 0) {
            raw.append(buf, static_cast<size_t>(rc));
            header_end = raw.find("\r\n\r\n");
            if (raw.size() > 64 * 1024 && header_end == std::string::npos) {
                error = "HTTP headers too large";
                return false;
            }
        } else {
            if (BIO_should_retry(bio)) continue;
            error = "BIO_read failed before headers complete";
            return false;
        }
    }

    const std::string headers_blob = raw.substr(0, header_end);
    std::string body = raw.substr(header_end + 4);

    // Парсинг статуса
    {
        std::istringstream hs(headers_blob);
        std::string status_line;
        std::getline(hs, status_line);
        if (!status_line.empty() && status_line.back() == '\r') status_line.pop_back();
        out.status_line = status_line;
        // "HTTP/1.1 200 OK"
        size_t sp1 = status_line.find(' ');
        size_t sp2 = (sp1 == std::string::npos) ? std::string::npos : status_line.find(' ', sp1 + 1);
        if (sp1 != std::string::npos) {
            const std::string code = (sp2 == std::string::npos)
                ? status_line.substr(sp1 + 1)
                : status_line.substr(sp1 + 1, sp2 - sp1 - 1);
            try { out.status_code = std::stoi(code); } catch (...) { out.status_code = 0; }
        }
    }

    // Заголовки
    std::size_t content_length = 0;
    bool has_content_length = false;
    bool chunked = false;
    {
        std::istringstream hs(headers_blob);
        std::string line;
        std::getline(hs, line);
        while (std::getline(hs, line)) {
            if (!line.empty() && line.back() == '\r') line.pop_back();
            const auto colon = line.find(':');
            if (colon == std::string::npos) continue;
            const std::string name = to_lower(line.substr(0, colon));
            std::string value = line.substr(colon + 1);
            while (!value.empty() && (value.front() == ' ' || value.front() == '\t')) value.erase(value.begin());
            if (name == "content-length") {
                try { content_length = static_cast<size_t>(std::stoul(value)); has_content_length = true; }
                catch (...) { /* ignore */ }
            } else if (name == "transfer-encoding" && to_lower(value).find("chunked") != std::string::npos) {
                chunked = true;
            }
        }
    }

    // Дочитываем тело
    auto append_body = [&](const char* p, size_t n) -> bool {
        const size_t allowed = (body.size() < max_body_bytes_) ? (max_body_bytes_ - body.size()) : 0;
        const size_t take = std::min(allowed, n);
        body.append(p, take);
        if (take < n) {
            out.truncated = true;
            return false;  // прекращаем чтение
        }
        return true;
    };

    bool continue_reading = body.size() < max_body_bytes_;
    while (continue_reading) {
        if (has_content_length && body.size() >= content_length) break;
        const int rc = BIO_read(bio, buf, sizeof(buf));
        if (rc > 0) {
            if (!append_body(buf, static_cast<size_t>(rc))) break;
        } else if (rc == 0) {
            break;  // EOF
        } else {
            if (BIO_should_retry(bio)) continue;
            break;
        }
    }

    if (chunked) {
        // Простой best-effort де-чанкер
        std::string decoded;
        decoded.reserve(body.size());
        size_t i = 0;
        while (i < body.size()) {
            const auto crlf = body.find("\r\n", i);
            if (crlf == std::string::npos) break;
            const std::string size_hex = body.substr(i, crlf - i);
            size_t chunk_sz = 0;
            try { chunk_sz = std::stoul(size_hex, nullptr, 16); } catch (...) { break; }
            i = crlf + 2;
            if (chunk_sz == 0) break;
            if (i + chunk_sz > body.size()) break;
            decoded.append(body, i, chunk_sz);
            i += chunk_sz;
            if (i + 2 <= body.size()) i += 2;  // CRLF после чанка
            if (decoded.size() >= max_body_bytes_) { out.truncated = true; break; }
        }
        body.swap(decoded);
    }

    out.body = std::move(body);
    if (out.body.size() >= max_body_bytes_) out.truncated = true;
    return true;
}

}  // namespace gci
