//! Команды Tauri, доступные из фронтенда через `invoke()`.

use crate::dto::{InspectRequest, InspectResult};
use crate::ffi;

use base64::Engine as _;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("trust store not available: {0}")]
    TrustStore(String),
    #[error("ffi: {0}")]
    Ffi(#[from] ffi::FfiError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("internal: {0}")]
    Internal(String),
    #[error("network: {0}")]
    Network(String),
}

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub(crate) fn validate_url(url: &str) -> Result<(), CommandError> {
    let lower = url.trim().to_lowercase();
    if !lower.starts_with("https://") {
        return Err(CommandError::InvalidUrl("only https:// allowed".into()));
    }
    if url.len() > 2048 {
        return Err(CommandError::InvalidUrl("url too long".into()));
    }
    if url.chars().any(|c| c.is_control()) {
        return Err(CommandError::InvalidUrl("url contains control chars".into()));
    }
    Ok(())
}

/// Возвращает путь до каталога trust-store, поставляемого как resource.
///
/// На Android `BaseDirectory::Resource` возвращает asset URL вида
/// `asset://localhost/trust-store`, а не файловый путь — `Path::exists()`
/// вернёт false. Поэтому существование проверяем только на не-Android.
///
/// Используется только C++ ядром (default feature). Под `mock-core` /
/// `rust-core` функция не вызывается, поэтому помечаем как allow(dead_code).
#[cfg_attr(any(feature = "mock-core", feature = "rust-core"), allow(dead_code))]
fn resolve_trust_store(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    let resource = app
        .path()
        .resolve("trust-store", tauri::path::BaseDirectory::Resource)
        .map_err(|e| CommandError::TrustStore(e.to_string()))?;

    // On Android resources are bundled as APK assets — the path is a virtual
    // asset:// URL and `exists()` always returns false. Skip the check there;
    // the C++ core (OpenSSL) will get the path and open it via AAssetManager.
    #[cfg(not(target_os = "android"))]
    if !resource.exists() {
        return Err(CommandError::TrustStore(format!(
            "trust-store not found at {:?}",
            resource
        )));
    }

    Ok(resource)
}

#[tauri::command]
pub async fn check_site(
    app: tauri::AppHandle,
    url: String,
    load_html: bool,
) -> Result<InspectResult, CommandError> {
    validate_url(&url)?;

    // mock-core doesn't use the trust store at all — skip resolution to avoid
    // false errors on Android where asset:// paths don't pass exists() checks.
    #[cfg(not(feature = "mock-core"))]
    let trust_store_str = {
        let trust_store = resolve_trust_store(&app)?;
        trust_store.to_string_lossy().to_string()
    };
    #[cfg(feature = "mock-core")]
    let trust_store_str = String::new();
    let _ = &app; // suppress unused warning in mock-core

    let request = InspectRequest::new(&url, &trust_store_str, load_html);
    let payload = serde_json::to_string(&request)?;

    // TLS-handshake + парсинг — синхронная работа, уносим её в blocking-пул,
    // чтобы не блокировать event-loop Tauri.
    let json_out = tauri::async_runtime::spawn_blocking(move || ffi::call_inspect_url(&payload))
        .await
        .map_err(|e| CommandError::Internal(format!("join: {e}")))??;

    let parsed: InspectResult = serde_json::from_str(&json_out)?;
    Ok(parsed)
}

#[tauri::command]
pub fn core_version() -> String {
    ffi::core_version()
}

/// Содержимое `trust-store/manifest.json`, встроенное в бинарь на этапе
/// сборки. На Android и iOS Tauri-resources бывают доступны только как
/// `asset://` URL и не открываются обычным `std::fs::read`, поэтому самым
/// надёжным и кроссплатформенным решением является `include_str!`.
///
/// Обновление trust-store = пересборка приложения (что соответствует
/// модели поставки УЦ Минцифры через App Store / Google Play, см. ТЗ §9).
const TRUST_STORE_MANIFEST: &str =
    include_str!("../../trust-store/manifest.json");

/// Возвращает разобранный `trust-store/manifest.json`.
/// Используется экраном настроек: отображает версию trust-store, источник
/// и перечень корневых/промежуточных CA.
#[tauri::command]
pub fn trust_store_info() -> Result<serde_json::Value, CommandError> {
    let value: serde_json::Value = serde_json::from_str(TRUST_STORE_MANIFEST)?;
    Ok(value)
}

/// Сохраняет JSON-отчёт в папку Downloads.
/// Возвращает полный путь к сохранённому файлу.
#[tauri::command]
pub async fn save_report(filename: String, content: String) -> Result<String, CommandError> {
    let download_dir = get_download_dir()?;

    // Создаём каталог если его нет
    std::fs::create_dir_all(&download_dir).map_err(|e| {
        CommandError::Internal(format!("cannot create dir {:?}: {}", download_dir, e))
    })?;

    let file_path = download_dir.join(&filename);
    std::fs::write(&file_path, content.as_bytes()).map_err(|e| {
        CommandError::Internal(format!("write failed: {}", e))
    })?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Определяет каталог Downloads в зависимости от ОС.
fn get_download_dir() -> Result<PathBuf, CommandError> {
    // Android: /storage/emulated/0/Download
    #[cfg(target_os = "android")]
    {
        let path = PathBuf::from("/storage/emulated/0/Download");
        if path.exists() {
            return Ok(path);
        }
        // Fallback: app-specific external storage
        if let Ok(ext) = std::env::var("EXTERNAL_STORAGE") {
            return Ok(PathBuf::from(ext).join("Download"));
        }
        return Ok(PathBuf::from("/sdcard/Download"));
    }

    // Desktop: используем dirs crate
    #[cfg(not(target_os = "android"))]
    {
        dirs::download_dir()
            .ok_or_else(|| CommandError::Internal("cannot find Downloads directory".into()))
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Реальная проверка обновлений trust-store (ТЗ §9).
//
// По умолчанию trust-store встроен в бинарь и обновляется только через
// App Store / Google Play. Эта команда выполняет «честную» онлайн-сверку:
// скачивает официальные PEM с сайта УЦ Минцифры, считает SHA-256 от их
// DER-представления и сравнивает с встроенным манифестом.
//
// Если фингерпринты совпадают → пользователь видит «актуально».
// Если различаются → видит «доступна новая версия — обновите приложение».
// OTA-замена сертификатов не выполняется намеренно, чтобы сохранить
// модель «trust-store подписан вместе с приложением».
// ──────────────────────────────────────────────────────────────────────────

/// Описание одного источника, который мы проверяем онлайн.
struct UpdateSource {
    /// Логическое имя, отображается во фронте (`root`, `sub`).
    name: &'static str,
    /// Подкаталог встроенного манифеста: `roots` / `intermediates`.
    bundle_key: &'static str,
    /// Имя PEM-файла, по которому ищем встроенный fingerprint.
    bundle_file: &'static str,
    /// Официальный URL с PEM на сайте УЦ Минцифры России.
    url: &'static str,
}

const UPDATE_SOURCES: &[UpdateSource] = &[
    UpdateSource {
        name: "root",
        bundle_key: "roots",
        bundle_file: "roots/russian-trusted-root-ca.pem",
        url: "https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt",
    },
    UpdateSource {
        name: "sub",
        bundle_key: "intermediates",
        bundle_file: "intermediates/russian-trusted-sub-ca.pem",
        url: "https://gu-st.ru/content/lending/russian_trusted_sub_ca_pem.crt",
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckEntry {
    name: String,
    url: String,
    bundled_fingerprint: String,
    remote_fingerprint: Option<String>,
    matches_bundled: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    checked_at: String,
    bundled_version: String,
    entries: Vec<UpdateCheckEntry>,
    up_to_date: bool,
}

/// Парсит PEM-строку, возвращает первый DER-блок CERTIFICATE.
fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let mut in_cert = false;
    let mut b64 = String::new();
    for line in pem.lines() {
        let line = line.trim();
        if line.starts_with("-----BEGIN CERTIFICATE") {
            in_cert = true;
            continue;
        }
        if line.starts_with("-----END CERTIFICATE") {
            break;
        }
        if in_cert && !line.is_empty() {
            b64.push_str(line);
        }
    }
    if b64.is_empty() {
        return Err("no CERTIFICATE block in PEM".into());
    }
    base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .map_err(|e| format!("base64 decode: {}", e))
}

/// Форматирует bytes как `AA:BB:CC:...` (так же, как в manifest.json).
fn fmt_fingerprint(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(&format!("{:02X}", b));
    }
    out
}

/// Достаёт fingerprint из встроенного манифеста по `file`.
fn bundled_fingerprint(manifest: &serde_json::Value, key: &str, file: &str) -> Option<String> {
    let arr = manifest.get(key)?.as_array()?;
    for item in arr {
        if item.get("file").and_then(|f| f.as_str()) == Some(file) {
            return item
                .get("fingerprintSha256")
                .and_then(|f| f.as_str())
                .map(|s| s.to_string());
        }
    }
    None
}

/// Скачивает PEM по URL и возвращает SHA-256 от DER-представления сертификата.
async fn fetch_remote_fingerprint(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| format!("read body: {}", e))?;
    let der = pem_to_der(&body)?;
    let digest = Sha256::digest(&der);
    Ok(fmt_fingerprint(&digest))
}

#[tauri::command]
pub async fn check_trust_store_updates() -> Result<UpdateCheckResult, CommandError> {
    let manifest: serde_json::Value = serde_json::from_str(TRUST_STORE_MANIFEST)?;
    let bundled_version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let client = reqwest::Client::builder()
        .user_agent("GosCertInspector/1.0 (+update-check)")
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| CommandError::Network(e.to_string()))?;

    let mut entries = Vec::with_capacity(UPDATE_SOURCES.len());
    let mut all_match = true;

    for src in UPDATE_SOURCES {
        let bundled_fp =
            bundled_fingerprint(&manifest, src.bundle_key, src.bundle_file).unwrap_or_default();

        match fetch_remote_fingerprint(&client, src.url).await {
            Ok(remote_fp) => {
                let matches = remote_fp.eq_ignore_ascii_case(&bundled_fp);
                if !matches {
                    all_match = false;
                }
                entries.push(UpdateCheckEntry {
                    name: src.name.to_string(),
                    url: src.url.to_string(),
                    bundled_fingerprint: bundled_fp,
                    remote_fingerprint: Some(remote_fp),
                    matches_bundled: matches,
                    error: None,
                });
            }
            Err(err) => {
                all_match = false;
                entries.push(UpdateCheckEntry {
                    name: src.name.to_string(),
                    url: src.url.to_string(),
                    bundled_fingerprint: bundled_fp,
                    remote_fingerprint: None,
                    matches_bundled: false,
                    error: Some(err),
                });
            }
        }
    }

    Ok(UpdateCheckResult {
        checked_at: current_iso8601(),
        bundled_version,
        entries,
        up_to_date: all_match,
    })
}

/// Минимальный ISO-8601 без зависимости от `chrono` (которое у нас под фичей).
fn current_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Быстрый перевод UNIX-секунд в UTC YYYY-MM-DDTHH:MM:SSZ (алгоритм
    // Howard Hinnant'a, без внешних зависимостей).
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;

    // 1970-01-01 == day 0 в shifted-эпохе Hinnant'a (era 0 starts at 0000-03-01)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mo <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, mo, d, h, m, s
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_http_scheme() {
        let r = validate_url("http://example.com/");
        assert!(matches!(r, Err(CommandError::InvalidUrl(_))));
    }

    #[test]
    fn rejects_control_chars() {
        let r = validate_url("https://example.com/\nabc");
        assert!(matches!(r, Err(CommandError::InvalidUrl(_))));
    }

    #[test]
    fn rejects_overly_long_url() {
        let long = format!("https://example.com/{}", "a".repeat(3000));
        assert!(validate_url(&long).is_err());
    }

    #[test]
    fn accepts_valid_https() {
        assert!(validate_url("https://gosuslugi.ru/").is_ok());
    }

    #[test]
    fn embedded_trust_store_manifest_is_valid_json() {
        let v: serde_json::Value =
            serde_json::from_str(TRUST_STORE_MANIFEST).expect("manifest.json must be valid JSON");
        assert!(v.get("version").is_some(), "missing `version`");
        assert!(v.get("roots").and_then(|r| r.as_array()).is_some(), "missing `roots[]`");
        assert!(
            v.get("intermediates").and_then(|r| r.as_array()).is_some(),
            "missing `intermediates[]`"
        );
    }

    #[test]
    fn trust_store_info_returns_manifest() {
        let v = trust_store_info().expect("trust_store_info should succeed");
        assert!(v.get("issuer").is_some());
    }

    #[test]
    fn pem_to_der_extracts_certificate_body() {
        let pem = "-----BEGIN CERTIFICATE-----\nQUJDREVGRw==\n-----END CERTIFICATE-----\n";
        let der = pem_to_der(pem).expect("PEM must decode");
        assert_eq!(der, b"ABCDEFG");
    }

    #[test]
    fn pem_to_der_rejects_empty() {
        assert!(pem_to_der("garbage").is_err());
    }

    #[test]
    fn fmt_fingerprint_uses_colon_uppercase_hex() {
        assert_eq!(fmt_fingerprint(&[0xAB, 0xCD, 0x01]), "AB:CD:01");
    }

    #[test]
    fn bundled_fingerprint_returns_known_value() {
        let manifest: serde_json::Value = serde_json::from_str(TRUST_STORE_MANIFEST).unwrap();
        let fp = bundled_fingerprint(&manifest, "roots", "roots/russian-trusted-root-ca.pem")
            .expect("bundled fingerprint must exist");
        assert!(
            fp.starts_with("D2:6D:2D"),
            "unexpected bundled fingerprint: {fp}"
        );
    }

    #[test]
    fn current_iso8601_is_well_formed() {
        let s = current_iso8601();
        assert_eq!(s.len(), 20, "iso8601 must be 20 chars: {s}");
        assert!(s.ends_with('Z'));
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(7), Some('-'));
        assert_eq!(s.chars().nth(10), Some('T'));
    }
}
