# Архитектура

## Высокоуровневая схема

```text
┌─────────────────────────────────────┐
│  Vue 3 UI (frontend/)               │   только UI, no business logic
│  - HomeView, ResultView             │
│  - SiteSelector, CertificateCard,   │
│    ChainTree, HtmlViewer,           │
│    JsonExporter                     │
│  - composables/useCheckSite         │── invoke('check_site') ──┐
└─────────────────────────────────────┘                          │
                                                                 ▼
┌─────────────────────────────────────┐    Tauri 2 (mobile/desktop)
│  Rust (src-tauri/)                  │
│  - commands.rs (check_site)         │   валидация, доступ к resource
│  - dto.rs (Serde)                   │   контракт строго типизирован
│  - ffi.rs (extern "C")              │   управление памятью FFI
└──────────────┬──────────────────────┘
               │ inspect_url(json)  → JSON
               ▼
┌─────────────────────────────────────┐
│  C++ (cpp-core/) + OpenSSL 3        │
│  - tls_client (SSL_CTX, handshake)  │
│  - x509_parser (cert + chain)       │
│  - http_client (HTTP/1.1 GET)       │
│  - inspector (orchestration + JSON) │
└──────────────┬──────────────────────┘
               │  HTTPS (TLS 1.2/1.3)
               ▼
┌─────────────────────────────────────┐
│  gosuslugi.ru / nalog.gov.ru / ...  │
└─────────────────────────────────────┘
```

## Принципы разделения

* **UI** не знает ни о OpenSSL, ни о FFI. Только DTO + Tauri-команды.
* **Rust** — тонкий граничный слой: валидация ввода, путь до trust-store,
  безопасная упаковка/распаковка C-строки, перевод JSON ↔ DTO.
* **C++** — единственная точка работы с сетью и криптографией. Возвращает
  готовый JSON по фиксированному контракту.
* **Trust store** — отдельный артефакт, версионируется, упаковывается как
  resource Tauri и подгружается C++ при каждом запросе.

## Поток одного запроса

1. Пользователь нажимает «Проверить».
2. `useCheckSite.checkSite(url, {loadHtml})` → `invoke('check_site', { url, loadHtml })`.
3. `commands::check_site`:
   * проверяет, что URL начинается с `https://`, без управляющих символов и ≤ 2048;
   * резолвит путь до trust-store через `app.path().resolve("trust-store", Resource)`;
   * сериализует `InspectRequest` в JSON;
   * запускает `ffi::call_inspect_url` в `spawn_blocking`.
4. `ffi::call_inspect_url` зовёт `extern "C" inspect_url`, копирует ответ
   и **обязательно** освобождает C-память через `inspector_free_string`.
5. C++ `inspect_url`:
   * парсит JSON-запрос;
   * `TlsClient::connect` — SSL_CTX (TLS 1.2/1.3 only, без слабых шифров),
     загружает trust-store, делает SNI + handshake;
   * `X509Parser::from_cert` / `chain_from_ssl` — детальный разбор;
   * `X509Parser::check_hostname` — `X509_check_host`;
   * `chain_signed_by_mincifry` — поиск маркеров «Russian Trusted …»,
     «Минцифры …» в issuer end-entity и subject промежуточных/корневых;
   * `HttpClient::get` — HTTP/1.1 GET по этому же `BIO`, лимит 1 МБ;
   * собирает финальный JSON по контракту.
6. Rust парсит ответ в `InspectResult`, возвращает фронту.
7. UI открывает `ResultView` и рисует `CertificateCard`, `ChainTree`,
   `HtmlViewer` (sandbox iframe) и `JsonExporter`.

## Безопасность

| Угроза                  | Митигация                                                           |
| ----------------------- | ------------------------------------------------------------------- |
| Слабые шифры            | `SSL_CTX_set_min_proto_version=TLS1.2`, кастомный cipher list       |
| MITM через системный CA | Используется **только локальный trust-store** в C++ (без системного)|
| XSS из чужого HTML      | HTML рендерится в `<iframe sandbox="">` через `data:` URL           |
| Утечка данных           | Нет внешних вызовов — только `invoke()` локально                    |
| Buffer overflow в FFI   | Память выделяется/освобождается одной стороной (C++)                |
| Невалидный URL          | Валидация в Rust **и** в C++ (`parse_url`)                          |
