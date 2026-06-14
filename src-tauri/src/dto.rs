//! Сериализуемые DTO, точно соответствующие JSON-контракту из docs/api.md.
//!
//! Используем `#[serde(rename_all = "camelCase")]` для большинства полей,
//! но `validation.*` и `is_mintsifry_ca` оставляем в snake_case, как требует
//! исходный контракт.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub valid_from: String,
    pub valid_to: String,
    pub san: Vec<String>,
    pub cn: String,
    pub fingerprint_sha256: String,
    pub signature_algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainEntry {
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub valid_from: String,
    pub valid_to: String,
    pub fingerprint_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub hostname_ok: bool,
    pub chain_ok: bool,
    pub expired_ok: bool,
    pub mincifry_ca_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectResult {
    pub request_id: String,
    pub input_url: String,
    pub resolved_host: String,
    pub tls_version: String,
    #[serde(default)]
    pub tls_cipher: Option<String>,
    pub certificate: Option<Certificate>,
    pub chain: Vec<ChainEntry>,
    pub validation: Validation,
    /// Intentionally kept in snake_case per contract. Accept both variants.
    #[serde(rename = "is_mintsifry_ca", alias = "isMintsifryCa")]
    pub is_mintsifry_ca: bool,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub errors: Vec<ErrorEntry>,
}

/// Сериализуемый запрос к C++ ядру.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectRequest<'a> {
    pub request_id: String,
    pub url: &'a str,
    pub trust_store_path: &'a str,
    pub load_html: bool,
    pub timeout_ms: u32,
    pub max_html_bytes: u32,
}

impl<'a> InspectRequest<'a> {
    pub fn new(url: &'a str, trust_store_path: &'a str, load_html: bool) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().simple().to_string(),
            url,
            trust_store_path,
            load_html,
            timeout_ms: 15_000,
            max_html_bytes: 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_serializes_in_snake_case() {
        let v = Validation {
            hostname_ok: true,
            chain_ok: false,
            expired_ok: true,
            mincifry_ca_ok: true,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"hostname_ok\":true"));
        assert!(s.contains("\"chain_ok\":false"));
        assert!(s.contains("\"expired_ok\":true"));
        assert!(s.contains("\"mincifry_ca_ok\":true"));
    }

    #[test]
    fn inspect_result_roundtrip() {
        let r = InspectResult {
            request_id: "rid".into(),
            input_url: "https://gosuslugi.ru/".into(),
            resolved_host: "gosuslugi.ru".into(),
            tls_version: "TLS 1.3".into(),
            tls_cipher: Some("TLS_AES_128_GCM_SHA256".into()),
            certificate: None,
            chain: vec![],
            validation: Validation {
                hostname_ok: true,
                chain_ok: true,
                expired_ok: true,
                mincifry_ca_ok: true,
            },
            is_mintsifry_ca: true,
            html: String::new(),
            errors: vec![],
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: InspectResult = serde_json::from_str(&s).unwrap();
        assert_eq!(back.tls_version, "TLS 1.3");
        assert!(back.is_mintsifry_ca);
    }

    #[test]
    fn request_is_camel_case() {
        let r = InspectRequest::new("https://x", "/store", true);
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"trustStorePath\":\"/store\""));
        assert!(s.contains("\"loadHtml\":true"));
        assert!(s.contains("\"maxHtmlBytes\":"));
    }
}
