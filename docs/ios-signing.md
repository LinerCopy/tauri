# iOS-подпись и Apple Developer Account

Здесь — пошаговая инструкция, как **легально** собрать подписанный
`.ipa` GosCertInspector для iPhone, и какой минимальный аккаунт нужен.

## TL;DR — сравнение вариантов

| Сценарий                                          | Аккаунт                                  | Что можно                                                                                  | Стоимость   |
| ------------------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------ | ----------- |
| **Open-source CI (текущий)**                      | Apple ID не нужен                        | Собрать **unsigned** `.app` из `.xcarchive`; запустить только в симуляторе                  | $0          |
| **Личная установка на свой iPhone**               | Бесплатный Apple ID (Personal Team)      | Установить из Xcode на свой телефон; срок действия профиля 7 дней, надо переподписывать    | $0          |
| **TestFlight / App Store / нормальный CI-подпис** | Apple Developer Program (полноценный)    | Подписанные `.ipa` с distribution-сертификатом, рассылка тестерам, публикация              | **$99/год** |

> Бесплатным Apple ID **нельзя** делать CI-сборку с подписью: Xcode
> генерирует профиль только из UI на твоей машине и привязывает его к
> конкретному устройству. Поэтому CI остаётся unsigned-вариантом, а
> подписывать ты будешь локально в Xcode или после оплаты $99.

---

## Вариант A. Бесплатный Apple ID — установка на свой iPhone

Подходит, если хочешь просто потестить приложение на своём телефоне.

### Шаг 1. Apple ID

1. Если ещё нет — создай Apple ID на <https://appleid.apple.com> (обычная
   почта, имя, пароль, телефон).
2. Включи двухфакторку (Apple это требует для подписи).

### Шаг 2. Установи Xcode

1. Поставь свежий Xcode из Mac App Store (≥ 15.x).
2. Запусти Xcode → меню **Xcode → Settings → Accounts → +** → введи
   Apple ID. После входа появится **Personal Team** (твоё имя).

### Шаг 3. Сгенерируй iOS-проект Tauri

```bash
cd src-tauri
cargo install tauri-cli --version "^2.0" --locked   # один раз
cargo tauri ios init                                # без --ci, чтобы Xcode мог редактировать pbxproj
```

### Шаг 4. Открой проект в Xcode и подпиши

```bash
open src-tauri/gen/apple/gci-app.xcodeproj
```

В Xcode:
1. Слева сверху выбери target **gci-app_iOS**.
2. Вкладка **Signing & Capabilities**.
3. ✅ **Automatically manage signing**.
4. **Team** → выбери свой `<Имя> (Personal Team)`.
5. **Bundle Identifier** должен быть уникальным в рамках твоего Apple ID.
   Если `ru.goscertinspector.app` занят — измени на
   `ru.goscertinspector.app.<твой-ник>`.
6. Подключи iPhone по USB, разрешишь «Доверять компьютеру».
7. В тулбаре выбери своё устройство и нажми ▶ Run.
8. На телефоне: **Настройки → Основные → VPN и управление устройством**
   → выбери свой Apple ID профиль → **Доверять**.

Готово, приложение запущено. Через 7 дней профиль протухнет —
надо будет пересобрать в Xcode тем же способом.

---

## Вариант B. Платный Apple Developer Program ($99/год) — CI-подпись

Подходит, если хочешь автоматическую сборку подписанного `.ipa` в
GitHub Actions, рассылку через TestFlight или публикацию в App Store.

### Шаг 1. Регистрация в Developer Program

1. <https://developer.apple.com/programs/enroll/>
2. Войди тем же Apple ID. Apple запросит:
   - Полное имя (как в паспорте).
   - Адрес, телефон.
   - Налоговый статус (Individual / Sole Proprietor — самое простое
     для одного разработчика без юр.лица).
3. Оплата $99 — карта или Apple Pay. На РФ-картах не работает,
   нужен зарубежный способ оплаты (или Revolut/Wise и т.п.).
4. Apple проверит данные обычно от 1 до 48 часов.

### Шаг 2. Узнай свой Team ID

После активации:
1. <https://developer.apple.com/account> → **Membership Details**.
2. Найди поле **Team ID** — 10 символов, формат `A1B2C3D4E5`.
3. Запомни — пригодится в Шаге 4.

### Шаг 3. Сгенерируй сертификат и provisioning profile

В Xcode (нужно сделать один раз на dev-машине):
1. **Settings → Accounts** → выбери свой Apple ID → **Manage Certificates**.
2. **+** → **Apple Development** (для разработки) или
   **Apple Distribution** (для TestFlight / App Store).
3. Открой проект в Xcode (как в Варианте A, Шаг 4).
4. **Signing & Capabilities** → **Team** = твоя команда (не Personal).
5. Xcode автоматически создаст provisioning profile и привяжет его.

Затем экспортируй сертификат, чтобы загрузить в CI:
1. **Keychain Access** → **My Certificates** → выбери только что
   созданный сертификат **Apple Development: \<Имя\>**.
2. Правый клик → **Export "Apple Development: ..."** → формат `.p12`,
   задай **пароль** (запомни — это будет `CERT_PASSWORD` в Actions).
3. Provisioning profile скачай с
   <https://developer.apple.com/account/resources/profiles/list>
   (`.mobileprovision`).

### Шаг 4. Добавь GitHub Secrets

В репозитории: **Settings → Secrets and variables → Actions → New
repository secret**. Создай:

| Secret              | Что туда положить                                                       |
| ------------------- | ----------------------------------------------------------------------- |
| `APPLE_TEAM_ID`     | 10-символьный Team ID из Шага 2                                          |
| `APPLE_CERT_P12`    | base64 от `.p12`-файла: `base64 -i dev.p12 \| pbcopy`                    |
| `APPLE_CERT_PASSWORD` | пароль, который ты задавал при экспорте `.p12`                         |
| `APPLE_PROVISIONING_PROFILE` | base64 от `.mobileprovision`: `base64 -i app.mobileprovision \| pbcopy` |
| `KEYCHAIN_PASSWORD` | любая строка (например, сгенерируй `openssl rand -hex 16`)              |

### Шаг 5. Допиши `.github/workflows/ios.yml`

Добавь шаги **перед** `cargo tauri ios build`:

```yaml
      - name: Import code signing certs
        env:
          CERT_B64: ${{ secrets.APPLE_CERT_P12 }}
          CERT_PASSWORD: ${{ secrets.APPLE_CERT_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
          PROFILE_B64: ${{ secrets.APPLE_PROVISIONING_PROFILE }}
        run: |
          set -euo pipefail
          # Создаём временный keychain
          security create-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security default-keychain -s build.keychain
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security set-keychain-settings -t 3600 -u build.keychain
          # Импорт .p12
          echo "$CERT_B64" | base64 --decode > /tmp/dev.p12
          security import /tmp/dev.p12 -k build.keychain \
            -P "$CERT_PASSWORD" -A -t cert -f pkcs12
          security set-key-partition-list -S apple-tool:,apple: \
            -s -k "$KEYCHAIN_PASSWORD" build.keychain
          # Установка provisioning profile
          mkdir -p ~/Library/MobileDevice/Provisioning\ Profiles
          echo "$PROFILE_B64" | base64 --decode \
            > ~/Library/MobileDevice/Provisioning\ Profiles/build.mobileprovision
```

Замени блок **Disable code signing for CI** на:

```yaml
      - name: Enable Xcode signing with team ID
        env:
          TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: |
          PROJ="src-tauri/gen/apple/gci-app.xcodeproj/project.pbxproj"
          sed -i '' "s/DEVELOPMENT_TEAM = \"\"/DEVELOPMENT_TEAM = $TEAM_ID/g" "$PROJ"
          sed -i '' "s/CODE_SIGN_IDENTITY = \"-\"/CODE_SIGN_IDENTITY = \"Apple Development\"/g" "$PROJ"
          sed -i '' '/CODE_SIGNING_REQUIRED = NO;/d' "$PROJ"
          sed -i '' '/CODE_SIGNING_ALLOWED = NO;/d' "$PROJ"
```

И уберай `continue-on-error: true` со сборки — теперь экспорт пройдёт.

### Шаг 6. Где `.ipa`?

Готовый `.ipa` появится в
`src-tauri/gen/apple/build/arm64/<scheme>.ipa` — добавь его в
`actions/upload-artifact`:

```yaml
      - name: Upload .ipa
        uses: actions/upload-artifact@v4
        with:
          name: goscertinspector-ipa
          path: src-tauri/gen/apple/build/**/*.ipa
```

---

## Что СЕЙЧАС делает наш CI

Без секретов / без аккаунта:
1. `cargo tauri ios build` собирает Rust + C++ + OpenSSL → создаётся
   `.xcarchive` (это шаг **успешен**).
2. `xcodebuild -exportArchive` падает на «No Team Found in Archive»
   — это ожидаемо, шаг помечен `continue-on-error: true`.
3. Следующий шаг **достаёт `.app` прямо из `.xcarchive`** и кладёт его
   в `src-tauri/gen/apple/build/Artifacts/`.
4. `actions/upload-artifact` поднимает этот `.app` как
   `goscertinspector-debug-ios`.

Этот `.app` работает в iOS Simulator (`xcrun simctl install booted
<path>.app`). Поставить на реальное устройство **без подписи нельзя** —
для этого нужен Apple ID + Xcode (Вариант A) либо $99 + CI-подпись
(Вариант B).

---

## Частые ошибки

| Ошибка                                                | Что делать                                                                                |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `error: exportArchive No Team Found in Archive`       | Без аккаунта — игнорируй (см. «Что СЕЙЧАС делает наш CI»). С аккаунтом — добавь `DEVELOPMENT_TEAM`. |
| `error: No profiles for 'ru.goscertinspector.app' were found` | Bundle ID должен быть зарегистрирован на твоём Apple ID. Поменяй в `tauri.conf.json`.    |
| `error: Provisioning profile ... doesn't include signing certificate` | `.p12` в keychain не совпадает с profile. Пересоздай profile в Xcode и пересохрани `.p12`. |
| `object file ... was built for newer iOS version (18.5) than being linked (14.0)` | Это **warning**, не error — Rust собирает .rlib с deployment-target новее, чем `minimumSystemVersion`. Можно игнорировать или поднять `iOS.minimumSystemVersion` в `tauri.conf.json` (например, до `17.0`). |
