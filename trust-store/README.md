# trust-store

Локальный набор PEM-сертификатов УЦ Минцифры, используемый C++ ядром через
`SSL_CTX_load_verify_locations`. Эти файлы упаковываются в бандл приложения
как Tauri resource (см. `src-tauri/tauri.conf.json → bundle.resources`).

## Структура

```
roots/             # корневые сертификаты (Russian Trusted Root CA)
intermediates/     # промежуточные сертификаты (Sub CA)
manifest.json      # версия и метаданные
```

## Получение актуальных PEM

Официальные ссылки Минцифры:

* `https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt`
* `https://gu-st.ru/content/lending/russian_trusted_sub_ca_pem.crt`

После скачивания положите файлы в соответствующие каталоги:

```bash
curl -fsSL https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt \
    -o trust-store/roots/russian_trusted_root_ca.pem
curl -fsSL https://gu-st.ru/content/lending/russian_trusted_sub_ca_pem.crt \
    -o trust-store/intermediates/russian_trusted_sub_ca.pem
```

Затем обновите `manifest.json`: `fingerprintSha256` и `notAfter` можно получить
через `openssl x509 -noout -fingerprint -sha256 -dates -in <file>`.

## Альтернатива: вытащить цепочку с живых сайтов

Для эксперимента/проверки можно вытащить промежуточные сертификаты прямо
с живых TLS-соединений:

```bash
./scripts/fetch-trust-store.sh gosuslugi.ru esia.gosuslugi.ru
```

Скрипт:

* делает `openssl s_client -showcerts -connect <host>:443`;
* разбивает PEM-бандл на отдельные файлы;
* пропускает leaf-сертификат;
* кладёт self-signed → `roots/`, остальные → `intermediates/`;
* имя файла: `<cn-slug>-<sha256[:12]>.pem`.

> ⚠️ **Не используйте в проде** сертификаты, полученные таким способом, если
> они не от УЦ Минцифры — они могут быть от Let's Encrypt или иностранного УЦ.
> Скрипт предназначен для отладки и валидации механизма загрузки trust-store.

## Проверка загруженных сертификатов

```bash
openssl x509 -in trust-store/roots/russian_trusted_root_ca.pem \
  -noout -subject -issuer -dates -fingerprint -sha256
```

Ожидаемое: CN содержит `Russian Trusted Root CA`, Subject == Issuer
(self-signed), `notAfter` — десятки лет вперёд.

## Подпись манифеста (опционально)

Для защищённого OTA-обновления подписывайте манифест ed25519-ключом и
кладите base64-подпись в поле `manifest.json → signature`. Проверку
подписи можно реализовать в Rust-команде `update_trust_store`
(вне MVP).
