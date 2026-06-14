#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Главная FFI-функция ядра. Принимает JSON-строку запроса и возвращает
 * указатель на C-строку с JSON-ответом по контракту, описанному в docs/api.md.
 *
 * Память возвращаемой строки выделяется внутри ядра. Вызывающая сторона
 * обязана освободить её через `inspector_free_string`.
 *
 * При любых внутренних ошибках возвращается валидный JSON с полем `errors`,
 * NULL не возвращается никогда.
 */
const char* inspect_url(const char* request_json);

/** Освобождение памяти, выделенной ядром под результат inspect_url. */
void inspector_free_string(const char* ptr);

/** Версия ядра (например, "1.0.0"). */
const char* inspector_version(void);

#ifdef __cplusplus
}
#endif
