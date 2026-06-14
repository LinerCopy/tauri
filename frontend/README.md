# frontend

Vue 3 + TypeScript + Vite UI, отрисовываемый Tauri WebView на Android / iOS.

## Команды

```bash
npm install
npm run dev      # Vite dev server (используется Tauri через beforeDevCommand)
npm run build    # типы + сборка → dist/
npm run test     # Vitest
npm run test:cov # покрытие
```

## Структура

```
src/
  components/       # презентационные компоненты
  composables/      # useCheckSite (вызов Tauri-команды)
  router/           # Vue Router (hash-history)
  types/            # DTO + список известных сайтов
  views/            # HomeView / ResultView
  App.vue, main.ts  # точка входа
tests/              # Vitest
```

## Соглашения по контракту

* поля валидации (`hostname_ok`, `chain_ok`, `expired_ok`, `mincifry_ca_ok`) —
  в snake_case **намеренно**, как и в исходном контракте;
* всё остальное — camelCase.

Подробнее — [`docs/api.md`](../docs/api.md).
