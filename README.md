# claude-lineup

Десктоп-приложение (Tauri 2) для сборки **harness-профилей** Claude Code:
какие MCP-серверы, плагины, skills, hooks и memory-файлы загрузит сессия.
Читает реальное состояние машины (`~/.claude`, `~/.claude.json`,
`<cwd>/.mcp.json`) и ничего не выдумывает: у каждой цифры — `measured`,
`estimated` или честное `unknown`.

- `src/ui/` — React 19 + Vite; `src/ui/api.ts` — единственный транспорт (Tauri invoke).
- `src-tauri/` — Rust-бэкенд; строки в `messages.json`, пути и env — в `config.rs`/`paths.rs`.

## Команды

| Команда | Что делает |
|---|---|
| `npx tauri dev` | приложение: окно + Vite на 5180 |
| `npx tauri build` | NSIS-инсталлятор |
| `npm test` | тесты Rust-бэкенда (cargo test) |
| `npx tsc --noEmit` | тайпчек UI |

## Сборка из исходников

Нужны: Rust (stable, edition 2024), Node.js ≥ 20, WebView2 (есть в Windows 10/11).

```bash
npm install
npx tauri dev     # запустить в режиме разработки
npx tauri build   # собрать NSIS-инсталлятор: src-tauri/target/release/bundle/nsis/
```

Готовый бинарник не требует ничего от машины, кроме WebView2: данные приложения
живут в `%APPDATA%\app.claudelineup.desktop`.

Правила проекта и архитектура: [CLAUDE.md](./CLAUDE.md).

## Лицензия

[MIT](./LICENSE).

