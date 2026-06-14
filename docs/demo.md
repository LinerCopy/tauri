# Demo: как показать приложение заказчику

В этом гайде — три способа продемонстрировать работу GosCertInspector,
от самого быстрого до полной мобильной сборки.

| Способ | Что показывает | Что нужно установить | Время |
|--------|----------------|-----------------------|-------|
| **A. Browser DEMO MODE** | Vue UI, навигацию, карточку сертификата, цепочку, HTML, экспорт JSON. Данные — mock. | Node 20+ | ~3 мин |
| **B. CLI на реальных сайтах** | Реальный TLS, peer cert, цепочка, флаг Минцифры. JSON по контракту. | Node 20+ | ~2 мин |
| **C. Tauri desktop preview** | UI + Rust commands. Native ядро по флагу `mock-core` или без него. | + Rust 1.77 (+OpenSSL для не-mock) | ~10 мин |
| **D. Полная мобильная сборка** | Production-сценарий: Android APK / iOS app | + JDK 17, NDK r26+, OpenSSL static, Xcode 15+ | ~1 час первый раз |

Для встречи с заказчиком используйте **A + B одновременно**:
A показывает реальный UI, B доказывает что контракт работает с реальными сайтами.

---

## A. Browser DEMO MODE (UI + mock backend)

### Запуск

```bash
cd frontend
npm install
npm run dev
```

---

## B. CLI на реальных гос-сайтах

CLI использует встроенный Node `tls`/`https` и **реально подключается к сайтам**,
формируя JSON по тому же контракту, что и C++ ядро.

### Запуск

```bash
# Без параметров — стандартный набор:
node scripts/check-sites.mjs

# С указанием URL:
node scripts/check-sites.mjs https://gosuslugi.ru https://nalog.gov.ru

# Без загрузки HTML (быстрее):
node scripts/check-sites.mjs --no-html

# С выгрузкой JSON в файлы:
node scripts/check-sites.mjs --out=reports/

# Справка:
node scripts/check-sites.mjs --help
```

### Что появится в консоли

```
GosCertInspector CLI — 5 site(s)
  → https://gosuslugi.ru ... 1832ms
  ...

═══ https://nalog.gov.ru ═══
  host:        nalog.gov.ru
  TLS:         TLS 1.3 (TLS_AES_256_GCM_SHA384)
  CN:          nalog.gov.ru
  Issuer:      C=US,O=Let's Encrypt,CN=YR2
  Valid:       2026-05-31T08:35:36Z → 2026-08-29T08:35:35Z
  SHA-256:     472DA3376148E07676633E59B6E73CD621B4D0326EDAF14703AF2D0293ADC038
  Chain depth: 4
  ✔ hostname_ok    ✔ chain_ok    ✔ expired_ok
  ○ is_mintsifry_ca = false

─── Сводка ───
 URL                       host    chain   expired   Минцифры
 https://gosuslugi.ru      ✔       ✔       ✔         ★ да
 https://nalog.gov.ru      ✔       ✔       ✔         ○ нет
 ...
```

### Файлы в `reports/`

После `--out=reports/`:

```
reports/
├── _summary.json             ← сводка по всем сайтам
├── gosuslugi.ru.json         ← полный отчёт по контракту
├── nalog.gov.ru.json
├── ...
```

### Возможные проблемы

* **`TLS_HANDSHAKE: timeout after 15000ms`** — сайт недоступен из вашей сети
  (часто гос-сайты блокируют не-российские IP). Решение: запуск из РФ-сети
  или через VPN, либо проверка через `curl -v https://<host>` для подтверждения.
* **`HOSTNAME_MISMATCH`** — реальный сертификат не валиден для домена. Это
  валидное наблюдение, не ошибка скрипта.
* **`is_mintsifry_ca = false` для реального гос-сайта** — означает, что в issuer
  действительно нет маркеров Минцифры. Многие сайты сейчас используют параллельно
  Let's Encrypt и Минцифры — провайдер балансирует, какой отдать.

---

## C. Tauri desktop preview

Когда нужно показать **взаимодействие Vue ↔ Rust commands**.

### C1. Без C++ — mock-core feature

Нужен только Rust 1.77+ и Tauri CLI.

```bash
cargo install tauri-cli --version "^2.0" --locked
cd src-tauri
GCI_SKIP_NATIVE=1 cargo tauri dev --features mock-core
```

Откроется нативное окно Tauri, в WebView отрисуется тот же Vue UI, но
данные пойдут через **реальный invoke → Rust command → mock-core (без C++)**.

### C2. С реальным C++ ядром (desktop)

Требует: cmake, clang/g++, OpenSSL 3 (через `brew install openssl@3` /
`apt install libssl-dev`).

```bash
brew install cmake openssl@3
# или
sudo apt install -y cmake libssl-dev

cd src-tauri
cargo tauri dev
```

При первом запуске CMake скачает nlohmann/json через FetchContent (нужен
интернет). Дальше всё кешируется.

### Положить trust-store перед запуском

⚠️ Без trust-store в режиме C2 любая проверка вернёт ошибку `EMPTY_TRUST` /
`trust store not available`. Сделайте один из вариантов:

```bash
# Вариант 1: официальные корни Минцифры
curl -fsSL https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt \
    -o trust-store/roots/russian_trusted_root_ca.pem
curl -fsSL https://gu-st.ru/content/lending/russian_trusted_sub_ca_pem.crt \
    -o trust-store/intermediates/russian_trusted_sub_ca.pem

# Вариант 2: вытащить с живого сайта (для отладки):
./scripts/fetch-trust-store.sh gosuslugi.ru
```

---

## D. Полная мобильная сборка

См. [`docs/build.md`](build.md). Кратко:

```bash
# 1) Соберите OpenSSL статически (см. build.md)
# 2) Установите Tauri CLI
cargo install tauri-cli --version "^2.0" --locked

# 3) Android
cd src-tauri
cargo tauri android init
export OPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/android-arm64
cargo tauri android dev
cargo tauri android build --apk

# 4) iOS (только macOS)
cargo tauri ios init
export OPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/ios-arm64
cargo tauri ios dev
cargo tauri ios build
```

---