//! Безопасная Rust-обёртка над C-ABI `inspector.h`.
//!
//! Три режима работы:
//! - `rust-core` (рекомендуемый) — реальный TLS на чистом Rust (rustls + x509-parser)
//! - `mock-core` — mock для тестов и демо
//! - default — FFI к C++/OpenSSL native библиотеке

#[cfg(not(any(feature = "mock-core", feature = "rust-core")))]
use std::ffi::CStr;
#[cfg(not(feature = "rust-core"))]
use std::ffi::CString;
#[cfg(not(any(feature = "mock-core", feature = "rust-core")))]
use std::os::raw::c_char;

#[cfg(not(any(feature = "mock-core", feature = "rust-core")))]
extern "C" {
    fn inspect_url(request_json: *const c_char) -> *const c_char;
    fn inspector_free_string(ptr: *const c_char);
    fn inspector_version() -> *const c_char;
}

#[derive(Debug, thiserror::Error)]
pub enum FfiError {
    #[error("invalid input: contains NUL byte")]
    NulInInput,
    #[error("native core returned null pointer")]
    NullResult,
    #[error("native core returned non-utf8 string")]
    NotUtf8,
}

/// Вызывает инспекцию URL и возвращает JSON-строку с результатом.
pub fn call_inspect_url(request_json: &str) -> Result<String, FfiError> {
    // Path 1: Pure Rust TLS (recommended for mobile)
    #[cfg(feature = "rust-core")]
    {
        Ok(crate::rust_core::inspect(request_json))
    }

    // Path 2: Mock (for tests/demo)
    #[cfg(all(feature = "mock-core", not(feature = "rust-core")))]
    {
        let _ = CString::new(request_json).map_err(|_| FfiError::NulInInput)?;
        Ok(mock::mock_inspect(request_json))
    }

    // Path 3: C++ FFI (native OpenSSL)
    #[cfg(not(any(feature = "mock-core", feature = "rust-core")))]
    {
        let c_in = CString::new(request_json).map_err(|_| FfiError::NulInInput)?;
        unsafe {
            let raw = inspect_url(c_in.as_ptr());
            if raw.is_null() {
                return Err(FfiError::NullResult);
            }
            let cstr = CStr::from_ptr(raw);
            let result = cstr.to_str().map(|s| s.to_owned()).map_err(|_| FfiError::NotUtf8);
            inspector_free_string(raw);
            result
        }
    }
}

/// Версия ядра.
pub fn core_version() -> String {
    #[cfg(feature = "rust-core")]
    {
        String::from("rust-core-1.0.0")
    }
    #[cfg(all(feature = "mock-core", not(feature = "rust-core")))]
    {
        String::from("mock-1.0.0")
    }
    #[cfg(not(any(feature = "mock-core", feature = "rust-core")))]
    {
        unsafe {
            let raw = inspector_version();
            if raw.is_null() {
                return String::from("unknown");
            }
            CStr::from_ptr(raw).to_string_lossy().into_owned()
        }
    }
}

#[cfg(feature = "mock-core")]
mod mock {
    use serde_json::{json, Value};

    /// Простейшая Rust-имитация ядра для unit-тестов и desktop preview без
    /// собранной native-библиотеки. Не делает реальный сетевой запрос.
    pub fn mock_inspect(request_json: &str) -> String {
        let req: Value = match serde_json::from_str(request_json) {
            Ok(v) => v,
            Err(e) => {
                return json!({
                    "requestId": "",
                    "inputUrl": "",
                    "resolvedHost": "",
                    "tlsVersion": "",
                    "certificate": null,
                    "chain": [],
                    "validation": {
                        "hostname_ok": false, "chain_ok": false,
                        "expired_ok": false, "mincifry_ca_ok": false
                    },
                    "is_mintsifry_ca": false,
                    "html": "",
                    "errors": [{"code":"BAD_JSON","message": e.to_string()}]
                })
                .to_string();
            }
        };
        let url = req.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let request_id = req
            .get("requestId")
            .and_then(|v| v.as_str())
            .unwrap_or("mock-rid")
            .to_string();
        json!({
            "requestId": request_id,
            "inputUrl": url,
            "resolvedHost": "mock.local",
            "tlsVersion": "TLS 1.3",
            "tlsCipher": "TLS_AES_128_GCM_SHA256",
            "certificate": {
                "subject": "CN=mock.local",
                "issuer": "CN=Russian Trusted Sub CA",
                "serialNumber": "01",
                "validFrom": "2024-01-01T00:00:00Z",
                "validTo":   "2026-01-01T00:00:00Z",
                "san": ["DNS:mock.local"],
                "cn": "mock.local",
                "fingerprintSha256": "DEADBEEF",
                "signatureAlgorithm": "sha256WithRSAEncryption"
            },
            "chain": [
                { "subject":"CN=mock.local",
                  "issuer":"CN=Russian Trusted Sub CA",
                  "serialNumber":"01",
                  "validFrom":"2024-01-01T00:00:00Z",
                  "validTo":"2026-01-01T00:00:00Z",
                  "fingerprintSha256":"DEADBEEF" },
                { "subject":"CN=Russian Trusted Sub CA",
                  "issuer":"CN=Russian Trusted Root CA",
                  "serialNumber":"02",
                  "validFrom":"2022-01-01T00:00:00Z",
                  "validTo":"2032-01-01T00:00:00Z",
                  "fingerprintSha256":"CAFEBABE" }
            ],
            "validation": {
                "hostname_ok": true, "chain_ok": true,
                "expired_ok": true,  "mincifry_ca_ok": true
            },
            "is_mintsifry_ca": true,
            "html": "<!doctype html><title>mock</title>",
            "errors": []
        })
        .to_string()
    }
}
