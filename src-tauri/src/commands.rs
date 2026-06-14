//! Команды Tauri, доступные из фронтенда через `invoke()`.

use crate::dto::{InspectRequest, InspectResult};
use crate::ffi;

use serde::Serialize;
use std::path::PathBuf;
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
}
