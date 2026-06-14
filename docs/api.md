# API: контракты и команды

## Tauri-команды

### `check_site`

```ts
invoke<InspectResult>('check_site', { url: string, loadHtml: boolean })
```

* `url` — обязателен, должен начинаться с `https://`, длина ≤ 2048,
  не содержит управляющих символов.
* `loadHtml` — загружать ли тело страницы (1 МБ лимит).

Ошибки команды возвращаются как rejected promise со строкой:
`invalid URL: ...`, `trust store not available: ...`, `ffi: ...`, `json: ...`.

### `core_version`

```ts
invoke<string>('core_version')
```

Возвращает строку с версией нативного C++ ядра.

## FFI

```c
// inspector.h
const char* inspect_url(const char* request_json);
void        inspector_free_string(const char* ptr);
const char* inspector_version(void);
```

Память возвращаемой строки выделяется **в ядре** через `malloc`
и должна быть освобождена через `inspector_free_string`.

## JSON-контракт запроса (Rust → C++)

```json
{
  "requestId":      "uuid-simple",
  "url":            "https://gosuslugi.ru/",
  "trustStorePath": "/path/to/trust-store",
  "loadHtml":       true,
  "timeoutMs":      15000,
  "maxHtmlBytes":   1048576
}
```

## JSON-контракт ответа (C++ → Rust → UI)

```json
{
  "requestId": "string",
  "inputUrl": "string",
  "resolvedHost": "string",
  "tlsVersion": "TLS 1.2 | TLS 1.3",
  "tlsCipher": "string (optional)",

  "certificate": {
    "subject": "string",
    "issuer": "string",
    "serialNumber": "string",
    "validFrom": "ISO8601",
    "validTo": "ISO8601",
    "san": ["string"],
    "cn": "string",
    "fingerprintSha256": "string",
    "signatureAlgorithm": "string"
  },

  "chain": [
    {
      "subject": "string",
      "issuer": "string",
      "serialNumber": "string",
      "validFrom": "ISO8601",
      "validTo": "ISO8601",
      "fingerprintSha256": "string"
    }
  ],

  "validation": {
    "hostname_ok":     true,
    "chain_ok":        true,
    "expired_ok":      true,
    "mincifry_ca_ok":  true
  },

  "is_mintsifry_ca": true,
  "html": "string",

  "errors": [
    { "code": "string", "message": "string" }
  ]
}
```

### Соглашение по регистру

* `validation.*` и `is_mintsifry_ca` — **snake_case** (исторический контракт).
* Все остальные поля — **camelCase**.

### Каталог кодов ошибок

| Код               | Когда возникает                                              |
| ----------------- | ------------------------------------------------------------ |
| `BAD_REQUEST`     | NULL `request_json` в FFI                                    |
| `BAD_JSON`        | request_json не парсится                                     |
| `EMPTY_URL`       | url пуст                                                     |
| `EMPTY_TRUST`     | trustStorePath пуст                                          |
| `URL_PARSE`       | scheme ≠ https, нет host, плохой порт                        |
| `TLS_HANDSHAKE`   | OpenSSL не смог установить соединение                        |
| `NO_PEER_CERT`    | Сервер не предъявил сертификат                               |
| `CHAIN_INVALID`   | `SSL_get_verify_result != X509_V_OK`                         |
| `HOSTNAME_MISMATCH`| `X509_check_host == 0`                                      |
| `EXPIRED`         | now вне `[validFrom, validTo]`                               |
| `HTTP_GET`        | Ошибка чтения/записи на HTTP-этапе                           |
| `HTTP_STATUS`     | HTTP-код ≥ 400                                               |
| `HTML_TRUNCATED`  | Тело обрезано до `maxHtmlBytes`                              |
