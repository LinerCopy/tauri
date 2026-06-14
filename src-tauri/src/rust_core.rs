//! Реализация TLS-инспекции на чистом Rust (rustls + x509-parser).
//! Не требует C++, OpenSSL или кросс-компиляции — компилируется нативно под Android.

use chrono::{DateTime, Utc};
use hex::encode as hex_encode;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use rustls_pemfile::certs;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use x509_parser::prelude::*;

/// Маркеры, по которым определяем сертификат Минцифры
const MINCIFRY_MARKERS: &[&str] = &[
    "russian trusted",
    "ministry of digital development",
    "минцифры",
    "минцифра",
];

/// Основная функция — принимает JSON-запрос, возвращает JSON-ответ по контракту.
pub fn inspect(request_json: &str) -> String {
    let req: Value = match serde_json::from_str(request_json) {
        Ok(v) => v,
        Err(e) => return error_response("", "", "BAD_JSON", &e.to_string()),
    };

    let url = req.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let request_id = req
        .get("requestId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let trust_store_path = req
        .get("trustStorePath")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let load_html = req.get("loadHtml").and_then(|v| v.as_bool()).unwrap_or(true);
    let timeout_ms = req.get("timeoutMs").and_then(|v| v.as_u64()).unwrap_or(15000) as u64;
    let max_html_bytes = req
        .get("maxHtmlBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(1_048_576) as usize;

    // Parse URL
    let host = match extract_host(url) {
        Some(h) => h,
        None => return error_response(&request_id, url, "INVALID_URL", "cannot extract host from URL"),
    };
    let port = extract_port(url).unwrap_or(443);

    // Build TLS config with custom trust store
    let tls_config = match build_tls_config(trust_store_path) {
        Ok(c) => c,
        Err(e) => return error_response(&request_id, url, "TRUST_STORE_ERROR", &e),
    };

    // TCP connect
    let addr = format!("{}:{}", host, port);
    let tcp = match TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| {
            // DNS resolution fallback
            use std::net::ToSocketAddrs;
            format!("{}:{}", host, port)
                .to_socket_addrs()
                .ok()
                .and_then(|mut addrs| addrs.next())
                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap())
        }),
        Duration::from_millis(timeout_ms),
    ) {
        Ok(s) => s,
        Err(_) => {
            // Fallback: try without timeout using ToSocketAddrs for DNS
            match connect_with_dns(&host, port, timeout_ms) {
                Ok(s) => s,
                Err(e) => {
                    return error_response(
                        &request_id,
                        url,
                        "TCP_CONNECT_FAILED",
                        &format!("cannot connect to {}: {}", addr, e),
                    )
                }
            }
        }
    };
    tcp.set_read_timeout(Some(Duration::from_millis(timeout_ms))).ok();
    tcp.set_write_timeout(Some(Duration::from_millis(timeout_ms))).ok();

    // TLS handshake
    let server_name = match ServerName::try_from(host.clone()) {
        Ok(sn) => sn,
        Err(e) => {
            return error_response(
                &request_id,
                url,
                "INVALID_HOSTNAME",
                &format!("invalid server name: {}", e),
            )
        }
    };

    let conn = match ClientConnection::new(tls_config, server_name) {
        Ok(c) => c,
        Err(e) => {
            return error_response(
                &request_id,
                url,
                "TLS_CONFIG_ERROR",
                &format!("TLS setup failed: {}", e),
            )
        }
    };

    let mut tls = StreamOwned::new(conn, tcp);

    // Send HTTP GET — this forces the TLS handshake
    let http_req = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: GosCertInspector/1.0\r\n\r\n",
        host
    );

    if let Err(e) = tls.write_all(http_req.as_bytes()) {
        return error_response(
            &request_id,
            url,
            "TLS_HANDSHAKE_FAILED",
            &format!("TLS write failed: {}", e),
        );
    }

    // Read response (for HTML)
    let mut response_bytes = Vec::new();
    if load_html {
        let mut buf = vec![0u8; 8192];
        let mut total = 0;
        loop {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    total += n;
                    response_bytes.extend_from_slice(&buf[..n]);
                    if total >= max_html_bytes {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    // Extract cert chain and TLS info from connection
    let peer_certs = tls.conn.peer_certificates().unwrap_or(&[]);

    let tls_version = match tls.conn.protocol_version() {
        Some(rustls::ProtocolVersion::TLSv1_2) => "TLS 1.2",
        Some(rustls::ProtocolVersion::TLSv1_3) => "TLS 1.3",
        _ => "unknown",
    };

    let tls_cipher = tls.conn
        .negotiated_cipher_suite()
        .map(|cs| format!("{:?}", cs.suite()))
        .unwrap_or_default();

    // Parse certs
    let mut certificate_json = Value::Null;
    let mut chain_json = Vec::new();
    let mut is_mintsifry = false;
    let mut hostname_ok = true;
    let mut chain_ok = !peer_certs.is_empty();
    let mut expired_ok = true;

    for (i, cert_der) in peer_certs.iter().enumerate() {
        let parsed = parse_cert(cert_der.as_ref());
        chain_json.push(parsed.clone());
        if i == 0 {
            certificate_json = parsed.clone();
            // Check hostname
            let cert_san = parsed.get("san").and_then(|v| v.as_array());
            if let Some(sans) = cert_san {
                let host_lower = host.to_lowercase();
                hostname_ok = sans.iter().any(|s| {
                    let san_str = s.as_str().unwrap_or("").to_lowercase();
                    let san_str = san_str.strip_prefix("dns:").unwrap_or(&san_str);
                    if san_str.starts_with("*.") {
                        // wildcard match
                        let suffix = &san_str[2..];
                        host_lower.ends_with(suffix)
                            && host_lower[..host_lower.len() - suffix.len()]
                                .chars()
                                .filter(|c| *c == '.')
                                .count()
                                == 0
                    } else {
                        san_str == host_lower
                    }
                });
            }
            // Check expiry
            let valid_to = parsed
                .get("validTo")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Ok(exp) = DateTime::parse_from_rfc3339(valid_to) {
                expired_ok = exp > Utc::now();
            }
        }
        // Check Mincifry markers in issuer
        let issuer = parsed
            .get("issuer")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let subject = parsed
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        for marker in MINCIFRY_MARKERS {
            if issuer.contains(marker) || subject.contains(marker) {
                is_mintsifry = true;
            }
        }
    }

    // Extract HTML body from HTTP response
    let html = if load_html {
        extract_html_body(&response_bytes)
    } else {
        String::new()
    };

    json!({
        "requestId": request_id,
        "inputUrl": url,
        "resolvedHost": host,
        "tlsVersion": tls_version,
        "tlsCipher": tls_cipher,
        "certificate": certificate_json,
        "chain": chain_json,
        "validation": {
            "hostname_ok": hostname_ok,
            "chain_ok": chain_ok,
            "expired_ok": expired_ok,
            "mincifry_ca_ok": is_mintsifry
        },
        "is_mintsifry_ca": is_mintsifry,
        "html": html,
        "errors": []
    })
    .to_string()
}

fn connect_with_dns(host: &str, port: u16, timeout_ms: u64) -> Result<TcpStream, String> {
    use std::net::ToSocketAddrs;
    let addr_str = format!("{}:{}", host, port);
    let addrs: Vec<_> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {}", e))?
        .collect();
    for addr in &addrs {
        match TcpStream::connect_timeout(addr, Duration::from_millis(timeout_ms)) {
            Ok(s) => return Ok(s),
            Err(_) => continue,
        }
    }
    Err(format!("all addresses failed for {}", addr_str))
}

fn build_tls_config(trust_store_path: &str) -> Result<Arc<ClientConfig>, String> {
    let mut root_store = RootCertStore::empty();

    // Add system/webpki roots as fallback
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    // Load custom trust-store certs (roots + intermediates)
    if !trust_store_path.is_empty() {
        let ts_path = Path::new(trust_store_path);
        for subdir in &["roots", "intermediates"] {
            let dir = ts_path.join(subdir);
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "pem" || e == "crt") {
                            if let Ok(data) = fs::read(&path) {
                                let mut reader = BufReader::new(data.as_slice());
                                for cert in certs(&mut reader).flatten() {
                                    let _ = root_store.add(cert);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

fn parse_cert(der: &[u8]) -> Value {
    match X509Certificate::from_der(der) {
        Ok((_, cert)) => {
            let subject = cert.subject().to_string();
            let issuer = cert.issuer().to_string();
            let serial = cert.raw_serial_as_string();
            let valid_from = cert.validity().not_before.to_rfc2822();
            let valid_to = cert.validity().not_after.to_rfc2822();

            // Convert to RFC3339
            let valid_from_rfc = asn1_to_rfc3339(cert.validity().not_before);
            let valid_to_rfc = asn1_to_rfc3339(cert.validity().not_after);

            // SAN
            let san: Vec<String> = cert
                .subject_alternative_name()
                .ok()
                .flatten()
                .map(|ext| {
                    ext.value
                        .general_names
                        .iter()
                        .map(|gn| match gn {
                            GeneralName::DNSName(s) => format!("DNS:{}", s),
                            GeneralName::IPAddress(b) => {
                                if b.len() == 4 {
                                    format!("IP:{}.{}.{}.{}", b[0], b[1], b[2], b[3])
                                } else {
                                    format!("IP:{:?}", b)
                                }
                            }
                            _ => format!("{:?}", gn),
                        })
                        .collect()
                })
                .unwrap_or_default();

            // CN
            let cn = cert
                .subject()
                .iter_common_name()
                .next()
                .and_then(|attr| attr.as_str().ok())
                .unwrap_or("")
                .to_string();

            // SHA-256 fingerprint
            let mut hasher = Sha256::new();
            hasher.update(der);
            let fingerprint = hex_encode(hasher.finalize());

            // Signature algorithm
            let sig_alg = cert.signature_algorithm.algorithm.to_string();

            json!({
                "subject": subject,
                "issuer": issuer,
                "serialNumber": serial,
                "validFrom": valid_from_rfc,
                "validTo": valid_to_rfc,
                "san": san,
                "cn": cn,
                "fingerprintSha256": fingerprint,
                "signatureAlgorithm": sig_alg
            })
        }
        Err(e) => {
            json!({
                "subject": "",
                "issuer": "",
                "serialNumber": "",
                "validFrom": "",
                "validTo": "",
                "san": [],
                "cn": "",
                "fingerprintSha256": "",
                "signatureAlgorithm": "",
                "parseError": e.to_string()
            })
        }
    }
}

fn asn1_to_rfc3339(time: x509_parser::time::ASN1Time) -> String {
    let ts = time.timestamp();
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

fn extract_host(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_port = without_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn extract_port(url: &str) -> Option<u16> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_port = without_scheme.split('/').next()?;
    let parts: Vec<&str> = host_port.splitn(2, ':').collect();
    if parts.len() == 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

fn extract_html_body(response: &[u8]) -> String {
    let text = String::from_utf8_lossy(response);
    // Find end of HTTP headers
    if let Some(pos) = text.find("\r\n\r\n") {
        text[pos + 4..].to_string()
    } else if let Some(pos) = text.find("\n\n") {
        text[pos + 2..].to_string()
    } else {
        text.to_string()
    }
}

fn error_response(request_id: &str, url: &str, code: &str, message: &str) -> String {
    json!({
        "requestId": request_id,
        "inputUrl": url,
        "resolvedHost": "",
        "tlsVersion": "",
        "tlsCipher": "",
        "certificate": null,
        "chain": [],
        "validation": {
            "hostname_ok": false,
            "chain_ok": false,
            "expired_ok": false,
            "mincifry_ca_ok": false
        },
        "is_mintsifry_ca": false,
        "html": "",
        "errors": [{"code": code, "message": message}]
    })
    .to_string()
}
