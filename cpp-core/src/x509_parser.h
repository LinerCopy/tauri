#pragma once

#include <openssl/ssl.h>
#include <openssl/x509.h>

#include <string>
#include <vector>

namespace gci {

struct CertInfo {
    std::string subject;
    std::string issuer;
    std::string serial_number;
    std::string valid_from;          // ISO8601 UTC
    std::string valid_to;            // ISO8601 UTC
    std::vector<std::string> san;
    std::string cn;
    std::string fingerprint_sha256;  // hex, uppercase, no separators
    std::string signature_algorithm;
    bool is_self_signed{false};
};

class X509Parser {
public:
    /** Извлекает информацию из end-entity сертификата. */
    static CertInfo from_cert(X509* cert);

    /**
     * Возвращает верифицированную цепочку: end-entity первой, далее
     * intermediates и root (если есть). Если verified chain недоступна,
     * откатывается на peer cert chain (без root).
     */
    static std::vector<CertInfo> chain_from_ssl(SSL* ssl);

    /** Проверка hostname через X509_check_host. */
    static bool check_hostname(X509* cert, const std::string& host);
};

}  // namespace gci
