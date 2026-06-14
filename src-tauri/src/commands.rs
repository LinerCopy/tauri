//! Команды Tauri, доступные из фронтенда через `invoke()`.

use crate::dto::{InspectRequest, InspectResult};
use crate::ffi;

use base64::Engine as _;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
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

// ──────────────────────────────────────────────────────────────────────────────
// Trust-store: встроенные в бинарь PEM-сертификаты + манифест.
//
// На Android/iOS Tauri-ресурсы доступны только как `asset://` URL,
// а C++ и Rust-ядро ожидают обычный файловый путь. Поэтому при первом
// запуске мы извлекаем embedded PEM в writable-каталог приложения
// (app data dir), а позже можем обновить их OTA.
// ──────────────────────────────────────────────────────────────────────────────

const TRUST_STORE_MANIFEST: &str =
    include_str!("../../trust-store/manifest.json");

const BUNDLED_ROOT_PEM: &[u8] =
    include_bytes!("../../trust-store/roots/russian-trusted-root-ca.pem");

const BUNDLED_SUB_PEM: &[u8] =
    include_bytes!("../../trust-store/intermediates/russian-trusted-sub-ca.pem");

/// Глобальный путь к writable trust-store каталогу, инициализируется один раз.
static TRUST_STORE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Инициализирует trust-store на файловой системе: создаёт каталоги roots/
/// и intermediates/, записывает в них встроенные PEM-файлы если они ещё
/// отсутствуют на диске (первый запуск или wipe кэша).
///
/// Вызывается из `check_site` лениво при первом обращении.
fn ensure_trust_store(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    if let Some(dir) = TRUST_STORE_DIR.get() {
        return Ok(dir.clone());
    }

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| CommandError::TrustStore(format!("no app data dir: {}", e)))?;

    let ts_dir = app_data.join("trust-store");
    let roots_dir = ts_dir.join("roots");
    let inter_dir = ts_dir.join("intermediates");

    fs::create_dir_all(&roots_dir).map_err(|e| {
        CommandError::TrustStore(format!("mkdir roots: {}", e))
    })?;
    fs::create_dir_all(&inter_dir).map_err(|e| {
        CommandError::TrustStore(format!("mkdir intermediates: {}", e))
    })?;

    // Записываем встроенные PEM если файлов нет (первый запуск).
    let root_path = roots_dir.join("russian-trusted-root-ca.pem");
    let sub_path = inter_dir.join("russian-trusted-sub-ca.pem");

    if !root_path.exists() {
        fs::write(&root_path, BUNDLED_ROOT_PEM).map_err(|e| {
            CommandError::TrustStore(format!("write root pem: {}", e))
        })?;
    }
    if !sub_path.exists() {
        fs::write(&sub_path, BUNDLED_SUB_PEM).map_err(|e| {
            CommandError::TrustStore(format!("write sub pem: {}", e))
        })?;
    }

    // Записываем manifest.json (для совместимости, хотя ядро его не читает).
    let manifest_path = ts_dir.join("manifest.json");
    if !manifest_path.exists() {
        fs::write(&manifest_path, TRUST_STORE_MANIFEST.as_bytes()).map_err(|e| {
            CommandError::TrustStore(format!("write manifest: {}", e))
        })?;
    }

    let _ = TRUST_STORE_DIR.set(ts_dir.clone());
    Ok(ts_dir)
}

/// Возвращает trust-store путь: writable каталог на Android/iOS,
/// обычный resource-путь на desktop.
#[cfg_attr(any(feature = "mock-core", feature = "rust-core"), allow(dead_code))]
fn resolve_trust_store(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    // На мобильных платформах (и desktop для единообразия) используем
    // writable app-data каталог с извлечёнными PEM.
    ensure_trust_store(app)
}

#[tauri::command]
pub async fn check_site(
    app: tauri::AppHandle,
    url: String,
    load_html: bool,
) -> Result<InspectResult, CommandError> {
    validate_url(&url)?;

    #[cfg(not(feature = "mock-core"))]
    let trust_store_str = {
        let trust_store = resolve_trust_store(&app)?;
        trust_store.to_string_lossy().to_string()
    };
    #[cfg(feature = "mock-core")]
    let trust_store_str = String::new();
    let _ = &app;

    let request = InspectRequest::new(&url, &trust_store_str, load_html);
    let payload = serde_json::to_string(&request)?;

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

/// Возвращает разобранный trust-store manifest.
/// Если есть обновлённый manifest на диске — читает его.
/// Иначе — встроенный.
#[tauri::command]
pub fn trust_store_info(app: tauri::AppHandle) -> Result<serde_json::Value, CommandError> {
    // Попытка прочитать живой manifest с диска (OTA-обновлённый).
    if let Ok(ts_dir) = ensure_trust_store(&app) {
        let manifest_path = ts_dir.join("manifest.json");
        if manifest_path.exists() {
            if let Ok(content) = fs::read_to_string(&manifest_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    return Ok(v);
                }
            }
        }
    }
    // Fallback: встроенный манифест.
    let value: serde_json::Value = serde_json::from_str(TRUST_STORE_MANIFEST)?;
    Ok(value)
}

/// Сохраняет JSON-отчёт в папку Downloads.
#[tauri::command]
pub async fn save_report(filename: String, content: String) -> Result<String, CommandError> {
    let download_dir = get_download_dir()?;
    fs::create_dir_all(&download_dir).map_err(|e| {
        CommandError::Internal(format!("cannot create dir {:?}: {}", download_dir, e))
    })?;
    let file_path = download_dir.join(&filename);
    fs::write(&file_path, content.as_bytes()).map_err(|e| {
        CommandError::Internal(format!("write failed: {}", e))
    })?;
    Ok(file_path.to_string_lossy().to_string())
}

fn get_download_dir() -> Result<PathBuf, CommandError> {
    #[cfg(target_os = "android")]
    {
        let path = PathBuf::from("/storage/emulated/0/Download");
        if path.exists() {
            return Ok(path);
        }
        if let Ok(ext) = std::env::var("EXTERNAL_STORAGE") {
            return Ok(PathBuf::from(ext).join("Download"));
        }
        return Ok(PathBuf::from("/sdcard/Download"));
    }
    #[cfg(not(target_os = "android"))]
    {
        dirs::download_dir()
            .ok_or_else(|| CommandError::Internal("cannot find Downloads directory".into()))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// OTA-обновление trust-store.
//
// При нажатии «Проверить обновление» бэкенд скачивает официальные PEM-файлы
// с сайта УЦ Минцифры, проверяет что они содержат валидный CERTIFICATE-блок,
// считает SHA-256 и, если фингерпринт отличается от текущего на диске,
// ПЕРЕЗАПИСЫВАЕТ локальные PEM (в writable app data) и обновляет manifest.json.
// Таким образом следующий вызов check_site уже будет использовать новые серты.
// ──────────────────────────────────────────────────────────────────────────────

struct UpdateSource {
    name: &'static str,
    bundle_key: &'static str,
    bundle_file: &'static str,
    /// Относительный путь PEM внутри trust-store каталога (для записи).
    rel_path: &'static str,
    url: &'static str,
}

const UPDATE_SOURCES: &[UpdateSource] = &[
    UpdateSource {
        name: "root",
        bundle_key: "roots",
        bundle_file: "roots/russian-trusted-root-ca.pem",
        rel_path: "roots/russian-trusted-root-ca.pem",
        url: "https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt",
    },
    UpdateSource {
        name: "sub",
        bundle_key: "intermediates",
        bundle_file: "intermediates/russian-trusted-sub-ca.pem",
        rel_path: "intermediates/russian-trusted-sub-ca.pem",
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
    updated: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    checked_at: String,
    bundled_version: String,
    entries: Vec<UpdateCheckEntry>,
    up_to_date: bool,
    /// Сколько сертификатов было реально обновлено на диске.
    certs_updated: u32,
}

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

/// Скачивает PEM по URL. Возвращает (текст PEM, SHA-256 fingerprint DER).
async fn fetch_remote_pem(
    client: &reqwest::Client,
    url: &str,
) -> Result<(String, String), String> {
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
    Ok((body, fmt_fingerprint(&digest)))
}

/// Читает текущий локальный PEM (с диска) и вычисляет его fingerprint.
fn local_fingerprint(ts_dir: &PathBuf, rel_path: &str) -> Option<String> {
    let path = ts_dir.join(rel_path);
    let pem = fs::read_to_string(&path).ok()?;
    let der = pem_to_der(&pem).ok()?;
    let digest = Sha256::digest(&der);
    Some(fmt_fingerprint(&digest))
}

#[tauri::command]
pub async fn check_trust_store_updates(
    app: tauri::AppHandle,
) -> Result<UpdateCheckResult, CommandError> {
    let ts_dir = ensure_trust_store(&app)?;

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
    let mut certs_updated: u32 = 0;

    for src in UPDATE_SOURCES {
        // Сначала определяем текущий fingerprint — он может быть уже
        // OTA-обновлённым (отличается от bundled).
        let current_fp = local_fingerprint(&ts_dir, src.rel_path)
            .or_else(|| bundled_fingerprint(&manifest, src.bundle_key, src.bundle_file))
            .unwrap_or_default();

        match fetch_remote_pem(&client, src.url).await {
            Ok((pem_text, remote_fp)) => {
                let matches = remote_fp.eq_ignore_ascii_case(&current_fp);
                let mut updated = false;

                if !matches {
                    // Новый сертификат — записываем на диск!
                    let dest = ts_dir.join(src.rel_path);
                    if let Some(parent) = dest.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    match fs::write(&dest, pem_text.as_bytes()) {
                        Ok(_) => {
                            updated = true;
                            certs_updated += 1;
                        }
                        Err(e) => {
                            entries.push(UpdateCheckEntry {
                                name: src.name.to_string(),
                                url: src.url.to_string(),
                                bundled_fingerprint: current_fp,
                                remote_fingerprint: Some(remote_fp),
                                matches_bundled: false,
                                updated: false,
                                error: Some(format!("write failed: {}", e)),
                            });
                            all_match = false;
                            continue;
                        }
                    }
                }

                entries.push(UpdateCheckEntry {
                    name: src.name.to_string(),
                    url: src.url.to_string(),
                    bundled_fingerprint: current_fp,
                    remote_fingerprint: Some(remote_fp),
                    matches_bundled: matches,
                    updated,
                    error: None,
                });
                if !matches && !updated {
                    all_match = false;
                }
            }
            Err(err) => {
                all_match = false;
                entries.push(UpdateCheckEntry {
                    name: src.name.to_string(),
                    url: src.url.to_string(),
                    bundled_fingerprint: current_fp,
                    remote_fingerprint: None,
                    matches_bundled: false,
                    updated: false,
                    error: Some(err),
                });
            }
        }
    }

    // Если были реальные обновления — обновим и manifest.json на диске,
    // чтобы trust_store_info возвращал свежие fingerprint.
    if certs_updated > 0 {
        update_local_manifest(&ts_dir);
    }

    Ok(UpdateCheckResult {
        checked_at: current_iso8601(),
        bundled_version,
        entries,
        up_to_date: all_match || certs_updated > 0,
        certs_updated,
    })
}

/// Перегенерирует `manifest.json` на диске из реально лежащих PEM.
fn update_local_manifest(ts_dir: &PathBuf) {
    // Читаем исходный шаблон
    let mut manifest: serde_json::Value = match serde_json::from_str(TRUST_STORE_MANIFEST) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Обновляем updatedAt
    if let Some(obj) = manifest.as_object_mut() {
        obj.insert("updatedAt".to_string(), serde_json::json!(current_iso8601()));
    }

    // Обновляем fingerprint для каждого PEM, если файл есть на диске
    for src in UPDATE_SOURCES {
        let pem_path = ts_dir.join(src.rel_path);
        if let Ok(pem) = fs::read_to_string(&pem_path) {
            if let Ok(der) = pem_to_der(&pem) {
                let fp = fmt_fingerprint(&Sha256::digest(&der));
                // Найдём и обновим запись в manifest
                if let Some(arr) = manifest.get_mut(src.bundle_key).and_then(|v| v.as_array_mut())
                {
                    for item in arr.iter_mut() {
                        if item.get("file").and_then(|f| f.as_str()) == Some(src.bundle_file) {
                            if let Some(obj) = item.as_object_mut() {
                                obj.insert(
                                    "fingerprintSha256".to_string(),
                                    serde_json::json!(fp),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Записываем обновлённый manifest
    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = fs::write(ts_dir.join("manifest.json"), json.as_bytes());
    }
}

fn current_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;

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
    fn embedded_root_pem_is_valid() {
        let pem = std::str::from_utf8(BUNDLED_ROOT_PEM).expect("root PEM must be UTF-8");
        let der = pem_to_der(pem).expect("root PEM must parse");
        assert!(der.len() > 100, "root DER too short: {} bytes", der.len());
    }

    #[test]
    fn embedded_sub_pem_is_valid() {
        let pem = std::str::from_utf8(BUNDLED_SUB_PEM).expect("sub PEM must be UTF-8");
        let der = pem_to_der(pem).expect("sub PEM must parse");
        assert!(der.len() > 100, "sub DER too short: {} bytes", der.len());
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
