#include "x509_parser.h"

#include <openssl/asn1.h>
#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/sha.h>
#include <openssl/x509v3.h>

#include <array>
#include <cstring>
#include <ctime>
#include <sstream>
#include <iomanip>

namespace gci {

namespace {

std::string x509_name_to_string(X509_NAME* name) {
    if (!name) return {};
    BIO* bio = BIO_new(BIO_s_mem());
    X509_NAME_print_ex(bio, name, 0,
        XN_FLAG_RFC2253 & ~ASN1_STRFLGS_ESC_MSB);
    char* data = nullptr;
    long len = BIO_get_mem_data(bio, &data);
    std::string out(data ? data : "", len > 0 ? static_cast<size_t>(len) : 0);
    BIO_free(bio);
    return out;
}

std::string asn1_time_to_iso(const ASN1_TIME* t) {
    if (!t) return {};
    struct tm tm{};
    if (ASN1_TIME_to_tm(t, &tm) != 1) return {};
    char buf[32];
    std::strftime(buf, sizeof(buf), "%Y-%m-%dT%H:%M:%SZ", &tm);
    return std::string(buf);
}

std::string bn_to_hex(const ASN1_INTEGER* serial) {
    if (!serial) return {};
    BIGNUM* bn = ASN1_INTEGER_to_BN(serial, nullptr);
    if (!bn) return {};
    char* hex = BN_bn2hex(bn);
    std::string out(hex ? hex : "");
    if (hex) OPENSSL_free(hex);
    BN_free(bn);
    // Нормализуем регистр
    for (auto& c : out) c = static_cast<char>(std::toupper(c));
    return out;
}

std::string get_cn(X509_NAME* name) {
    if (!name) return {};
    char buf[256];
    int len = X509_NAME_get_text_by_NID(name, NID_commonName, buf, sizeof(buf));
    if (len <= 0) return {};
    return std::string(buf, static_cast<size_t>(len));
}

std::vector<std::string> get_san(X509* cert) {
    std::vector<std::string> result;
    if (!cert) return result;
    auto* names = static_cast<GENERAL_NAMES*>(
        X509_get_ext_d2i(cert, NID_subject_alt_name, nullptr, nullptr));
    if (!names) return result;
    const int count = sk_GENERAL_NAME_num(names);
    for (int i = 0; i < count; ++i) {
        const GENERAL_NAME* gn = sk_GENERAL_NAME_value(names, i);
        if (!gn) continue;
        if (gn->type == GEN_DNS) {
            const unsigned char* data = ASN1_STRING_get0_data(gn->d.dNSName);
            const int len = ASN1_STRING_length(gn->d.dNSName);
            if (data && len > 0) {
                result.emplace_back("DNS:" + std::string(reinterpret_cast<const char*>(data),
                                                          static_cast<size_t>(len)));
            }
        } else if (gn->type == GEN_IPADD) {
            const unsigned char* ip = ASN1_STRING_get0_data(gn->d.iPAddress);
            const int len = ASN1_STRING_length(gn->d.iPAddress);
            std::ostringstream oss;
            oss << "IP:";
            if (len == 4) {
                for (int j = 0; j < 4; ++j) {
                    if (j) oss << ".";
                    oss << static_cast<int>(ip[j]);
                }
            } else if (len == 16) {
                oss << std::hex;
                for (int j = 0; j < 16; j += 2) {
                    if (j) oss << ":";
                    oss << ((ip[j] << 8) | ip[j + 1]);
                }
            }
            result.emplace_back(oss.str());
        }
    }
    GENERAL_NAMES_free(names);
    return result;
}

std::string fingerprint_sha256(X509* cert) {
    if (!cert) return {};
    std::array<unsigned char, EVP_MAX_MD_SIZE> buf{};
    unsigned int len = 0;
    if (X509_digest(cert, EVP_sha256(), buf.data(), &len) != 1) return {};
    std::ostringstream oss;
    oss << std::hex << std::uppercase << std::setfill('0');
    for (unsigned int i = 0; i < len; ++i) {
        oss << std::setw(2) << static_cast<int>(buf[i]);
    }
    return oss.str();
}

std::string signature_algorithm(X509* cert) {
    if (!cert) return {};
    const X509_ALGOR* alg = nullptr;
    X509_get0_signature(nullptr, &alg, cert);
    if (!alg) return {};
    char buf[128];
    OBJ_obj2txt(buf, sizeof(buf), alg->algorithm, 0);
    return std::string(buf);
}

bool detect_self_signed(X509* cert) {
    if (!cert) return false;
    return X509_NAME_cmp(X509_get_subject_name(cert), X509_get_issuer_name(cert)) == 0;
}

}  // namespace

CertInfo X509Parser::from_cert(X509* cert) {
    CertInfo info;
    if (!cert) return info;
    info.subject             = x509_name_to_string(X509_get_subject_name(cert));
    info.issuer              = x509_name_to_string(X509_get_issuer_name(cert));
    info.serial_number       = bn_to_hex(X509_get_serialNumber(cert));
    info.valid_from          = asn1_time_to_iso(X509_get0_notBefore(cert));
    info.valid_to            = asn1_time_to_iso(X509_get0_notAfter(cert));
    info.san                 = get_san(cert);
    info.cn                  = get_cn(X509_get_subject_name(cert));
    info.fingerprint_sha256  = fingerprint_sha256(cert);
    info.signature_algorithm = signature_algorithm(cert);
    info.is_self_signed      = detect_self_signed(cert);
    return info;
}

std::vector<CertInfo> X509Parser::chain_from_ssl(SSL* ssl) {
    std::vector<CertInfo> out;
    if (!ssl) return out;

    // Сначала пробуем verified chain (OpenSSL 1.1+).
    STACK_OF(X509)* chain = SSL_get0_verified_chain(ssl);
    if (!chain || sk_X509_num(chain) == 0) {
        // Fallback: peer chain (не содержит root).
        chain = SSL_get_peer_cert_chain(ssl);
    }
    if (!chain) return out;

    const int n = sk_X509_num(chain);
    out.reserve(static_cast<size_t>(n));
    for (int i = 0; i < n; ++i) {
        X509* c = sk_X509_value(chain, i);
        out.push_back(X509Parser::from_cert(c));
    }
    return out;
}

bool X509Parser::check_hostname(X509* cert, const std::string& host) {
    if (!cert || host.empty()) return false;
    return X509_check_host(cert, host.c_str(), host.size(), 0, nullptr) == 1;
}

}  // namespace gci
