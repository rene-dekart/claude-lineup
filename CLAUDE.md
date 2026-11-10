# claude-lineup

Desktop app (Tauri 2) building **harness layout profiles** for Claude Code (which
MCP servers, plugins, skills, hooks, memory files load), applied to a project —
writes `.claude/settings.json` — or to a session — prints a `claude` argv. Reads
real machine state only: `~/.claude`, `~/.claude.json`, `<cwd>/.mcp.json`. No mock
numbers.

Design doc (intent, Russian): `docs/superpowers/specs/2026-10-05-claude-lineup-design.md`.
Product name `claude-lineup` (`package.json`); folder keeps `claude-configurator` —
renaming breaks session paths.

## Commands

| Command | What it does |
|---|---|
| `npx tauri dev` | the app: Rust backend + Vite **5180** in the OS webview. |
| `npx tauri build` | NSIS installer. |
| `npm test` | `cargo test` in `src-tauri/` — 62 tests, ~2 s. |
| `npx tsc --noEmit` | Typecheck the UI. Clean. |
| `npm run build:ui` | `vite build` → `dist/ui` (what `tauri build` runs). |

## Architecture

`src-tauri/` — a normal Cargo project (edition 2024) plus `tauri.conf.json`,
`capabilities/`, `icons/`. `src/lib.rs` holds the app: one `#[tauri::command]` per
former web route, registered in `run()`; `src/main.rs` is the thin desktop entry
point the Tauri template generates. Modules mirror the old server's: `scan.rs` →
Inventory; `resolve.rs` → profile+inventory → `{ files | argv }`; `profiles.rs`
(deepMerge: arrays REPLACED, not appended); `cost.rs` (measured | estimated |
unknown); `stats.rs` (from transcripts, cache v5); `remote.rs` (skillselion) +
`mcpreg.rs` (MCP registry, disk-cached); `probe.rs` (live MCP measurement);
`install.rs` (claude CLI spawn). Strings live in `src-tauri/messages.json`
(`{ ru, en }`), read through `lang.rs`/`t()`.

`src/ui/` — React 19 + Vite, `src/ui/api.ts` the only transport (Tauri `invoke`).
`src/shared/` — browser-neutral types + the i18n dict the UI renders with.

The old TS server (`src/server/`, `test/`) was the porting reference; deleted
2026-11-09 after the parity checks. The Rust side is the only backend.

## Hard rules

Invariants; breaking one is a bug, not a style choice.

1. **Never write Claude Code's own files by hand.** `~/.claude.json`,
   `~/.claude/settings.json`, `~/.claude/plugins/installed_plugins.json` belong to
   the CLI and a running session rewrites them — an external write is a race with
   guaranteed loss. Read-only. Install = `claude plugin install …`
   (`src-tauri/src/install.rs`).
2. **Nothing written without an explicit `apply`.** `resolve` is a dry run returning
   a diff; `apply_files` refuses if the target changed since resolve and takes a
   restore point first (`src-tauri/src/backups.rs`).
3. **Never invent a number.** Each cost carries `level: 'measured' | 'estimated' |
   'unknown'`; `unknown` is the honest answer for hooks and unprobed MCP servers —
   a fake digit is worse than a dash.
4. **No source — no line.** `description` optional everywhere: skill ← frontmatter,
   plugin ← catalog cache else `plugin.json`, MCP ← owner plugin else probed tool
   names. No text composed to tidy a list.
5. **Provenance mandatory.** Each entity carries `{ file, line? }` — the scan only
   reads, so this is the only way to judge a row.
6. **`src-tauri/src/config.rs` is the only place env vars are read** (`env_str`/
   `env_int`; garbage → default, zero valid). Machine facts go there; contracts of
   Claude Code and skillselion responses do not — a typo there breaks parsing
   someone else's response.
7. **The paths root, not `process.cwd()`** — `paths::init` fixes it at startup
   (`lib.rs` setup; `CC_DATA_ROOT` override), `cd` must not move profiles and caches.
8. **External input validated before spawn.** repo `owner/repo`, plugin
   `id@marketplace` → `src-tauri/src/repourl.rs` character white-lists; `--yes`
   only after showing the human the command *and* its `sha256`. A `"` or `%` in argv
   is a refusal, not mangling (`%` expands in `cmd.exe` even quoted).
9. **Windows-first.** `claude` is `claude.cmd` → spawn `cmd /d /s /c`, `--settings`
   travels as a *file*, not a JSON string (cmd.exe strips quotes).
10. **Never delete or truncate `data/` or `profiles/`** to "clean up" — real user
    state; `data/` is gitignored for that reason.

## Error handling

Broken JSON in someone else's tree must not crash the app: snapshot-backup, mark
unreadable, continue. A network failure is `notes`, never an error surfaced as a
crash — the on-disk catalog works offline. Install succeeds only at `code === 0`
**and** no `shownCommand`: exit 0 with an unconfirmed command means it hasn't
happened.

## Testing

`cargo test` (in `src-tauri/`), `#[cfg(test)] mod tests` per module, no frameworks,
nothing on disk.

- Settings trees in a temp dir via `std::env::temp_dir()`, never `fixtures/` —
  fixtures drift from real `~/.claude.json` paths and start lying.
- Offline. `remote`, `catalog` against deliberately malformed responses (record
  without `repo` dropped, `stars: "many"` not a number). `resolve` golden:
  profile + inventory → exact files.
- Test titles and comments English. The only Russian strings in tests are the
  expected outputs of the ru locale itself.
- Temporary real-machine checks (compare against the deleted TS reference) are
  deleted after use — never left as permanent `#[ignore]` tests.
- The UI has no unit tests; `npx tsc --noEmit` is its gate.

## i18n (ru/en)

Every visible string goes through a dict as `{ ru, en }` with a flat,
area-prefixed key (`inv.*`, `sess.*`, `scan.*`, `net.*`, `core.*`, `ui.*`,
`prof.*`). No literals in components.

- UI: `src/shared/i18n/messages/*.ts` via a React context (`src/ui/i18n.tsx`).
  Rust: `src-tauri/messages.json` via `lang::t`; every command takes a `lang`
  argument from the UI. Missing header (tests, curl) → `ru`.
- Messages are templates, not concatenation. An unknown key renders *as the key*: a
  visible `inv.foo` is the error message.

## Style

Comments and identifiers English, design doc Russian. Comments explain *why*,
especially where the naive expectation is wrong (see `src-tauri/src/stats.rs` on
path sorting, `probe.rs` on UTF-16 lengths). `// ponytail: …` marks a deliberate
simplification — a known ceiling with its upgrade path. No prose in the UI: the
page shows numbers, the explanation lives in the answer.

## Known gaps

- Design doc §4.10 CLI (`claude-lineup <profile> --dry`) not implemented — no CLI
  entry point, no `bin` field. Other sections are.
- Not a git repository yet (`graphify-out/` freshness vs `HEAD` impossible).
- Design docs, `docs/harness/`, `docs/prototypes/`, `mockups/` are local
  reference, gitignored — not part of the public repository.
