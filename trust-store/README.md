# trust-store

Локальный набор PEM-сертификатов УЦ Минцифры, используемый C++ ядром через
`SSL_CTX_load_verify_locations`. Эти файлы упаковываются в бандл приложения
как Tauri resource.

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

## Подпись манифеста (опционально)

Для защищённого OTA-обновления подписывайте манифест ed25519-ключом и
кладите base64-подпись в поле `manifest.json → signature`. Проверку
подписи можно реализовать в Rust-команде `update_trust_store`
(вне MVP).
