# GosCertInspector

Кроссплатформенное мобильное приложение (Android 8.0+ / iOS 14+) для безопасного
подключения по TLS к государственным сайтам РФ, разбора серверных сертификатов и
цепочки доверия, а также определения того, выдан ли сертификат **удостоверяющим
центром Минцифры**.

## Стек

| Слой         | Технологии                                                                |
| ------------ | ------------------------------------------------------------------------- |
| UI           | Vue 3 + TypeScript + Vite + Vitest                                        |
| Mobile host  | Tauri 2.x (Android + iOS)                                                 |
| Native glue  | Rust (Tauri plugin + FFI)                                                 |
| Core         | C++17 + OpenSSL 3.x                                                       |
| Crypto       | TLS 1.2/1.3, X.509 разбор, проверка цепочки относительно локального store |
| Build        | CMake + NDK r26+ (Android), XCFramework + Xcode (iOS), OpenSSL static     |

## Архитектура (поток данных)

```text
[Vue UI] → invoke('check_site') → [Rust command]
                                        │
                                        ▼
                                 [FFI: inspect_url]
                                        │
                                        ▼
                          [C++ core: OpenSSL TLS/HTTP]
                                        │
                                        ▼
                                  HTTPS to gov.ru
```

Подробнее: [`docs/architecture.md`](docs/architecture.md).

## Структура репозитория

```text
frontend/      Vue 3 + TS + Vite + Vitest
src-tauri/     Tauri 2.x приложение и Rust команды
cpp-core/      C++17 ядро с OpenSSL и FFI
trust-store/   Локальный набор сертификатов Минцифры
docs/          Документация: architecture, api, build
.github/       CI workflows для Android и iOS
```

## Быстрый старт (desktop preview)

```bash
# 1) Frontend dev server
cd frontend && npm install && npm run dev

# 2) В соседнем терминале — Tauri (desktop preview, без нативного C++)
cd src-tauri && cargo tauri dev
```

Для мобильной сборки см. [`docs/build.md`](docs/build.md).

## Безопасность

* TLS 1.2/1.3 only; слабые шифры отключены на уровне SSL_CTX.
* Hostname verification через `X509_check_host`.
* Цепочка проверяется относительно **локального trust store**, а не системного.
* Данные не уходят за пределы устройства.
* HTML отрисовывается в `<iframe sandbox>`.

## Лицензия

MIT, см. [`LICENSE`](LICENSE).
