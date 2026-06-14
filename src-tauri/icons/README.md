# Иконки приложения

В этой папке должны находиться иконки, перечисленные в `tauri.conf.json → bundle.icon`.

Сгенерировать набор можно командой:

```bash
cd src-tauri
cargo tauri icon ../assets/source-icon.png
```

Это создаст:

* `32x32.png`, `128x128.png`, `128x128@2x.png`
* `icon.icns` (macOS), `icon.ico` (Windows)
* набор Android (`mipmap-*`) и iOS (`AppIcon.appiconset`) автоматически
  в каталогах `gen/android` / `gen/apple` после `tauri android init` / `tauri ios init`.

> Файлы коммитить не обязательно — они генерятся локально и в CI.
