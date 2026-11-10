import { invoke } from '@tauri-apps/api/core'
import type {
  Card,
  Catalog,
  ConfigFile,
  GeneratedDescription,
  InstallResult,
  Inventory,
  McpRegistry,
  ModuleRow,
  PickResult,
  Profile,
  RemoteCatalog,
  RemoteInstallResult,
  RemotePlan,
  Resolution,
  RestorePoint,
  Stats,
} from '../shared/types.js'
import type { Lang } from '../shared/i18n/index.js'

/**
 * The desktop transport: every call is an `invoke` into the Rust core, and
 * every call carries the UI language — server-side messages are localised
 * there, same rule as the web version's `x-lang` header. The UI sets it via
 * `setApiLang` whenever the toggle changes.
 */
let lang: Lang = 'ru'

export const setApiLang = (l: Lang): void => {
  lang = l
}

async function req<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, { lang, ...(args ?? {}) })
  } catch (e) {
    // Tauri rejects with the command's `Err(String)`; anything else is a bug.
    throw new Error(typeof e === 'string' ? e : ((e as Error)?.message ?? String(e)))
  }
}

export const getInventory = (cwd: string) => req<Inventory>('get_inventory', { cwd })

/**
 * Every settings file that shapes this project, exactly as it lies on disk —
 * the full-configuration reading, next to the one the profile would write.
 */
export const getConfigFiles = (cwd: string) =>
  req<{ files: ConfigFile[] }>('get_config_files', { cwd })

/**
 * `~/.claude.json` as a readable view — this project's entry plus the
 * machine-wide keys. `null` when the file is missing or unreadable.
 */
export const getClaudeJson = (cwd: string) =>
  req<{ file: ConfigFile | null }>('get_claude_json', { cwd })

export const getProfiles = () => req<Profile[]>('get_profiles')
export const saveProfile = (p: Profile) => req<Profile>('profile_save', { profile: p })
export const deleteProfile = (id: string) =>
  req<{ deleted: boolean }>('profile_delete', { id })

export const resolveProfile = (profile: Profile, cwd: string) =>
  req<{ resolution: Resolution }>('resolve_profile', { profile, cwd })

export const applyProfile = (profile: Profile, cwd: string) =>
  req<{
    written?: string[]
    backups?: string[]
    /** The restore point this write can be rolled back to. */
    point?: string | null
    warnings: string[]
  }>('apply_profile', { profile, cwd })

export const listBackups = () => req<{ points: RestorePoint[] }>('backups_list')

export const restoreBackup = (id: string) =>
  req<{ restored: string[]; deleted: string[]; point: string | null }>('backup_restore', { id })

/** Throw one point away. The files it describes are not touched. */
export const deleteBackup = (id: string) =>
  req<{ deleted: boolean }>('backup_delete', { id })

export const clearBackups = () => req<{ cleared: number }>('backups_clear')

export const probeMcp = (name: string) =>
  req<{ cost: { level: string; tokens?: number; note: string } }>('probe_mcp', { name })

/** Opens the OS folder dialog; the answer is the path itself. */
export const pickDir = (initial?: string) =>
  req<PickResult>('pick_dir', { initial: initial ?? null })

/**
 * What a card is asked about. A registry row answers with what it already
 * knows — its repository, description, version and transports — because none
 * of it is on disk for the card to read.
 */
export interface CardQueryUi {
  kind: string
  name: string
  cwd?: string
  file?: string
  scope?: string
  origin?: string
  repo?: string
  description?: string
  version?: string
  transport?: string
}

export const getCard = (q: CardQueryUi) => req<Card>('get_card', { ...q })

/**
 * A description for an entry whose author wrote none. Removed from the desktop
 * build together with the Ollama path; the export stays so the callers keep
 * compiling and get an honest refusal.
 */
export const describeCard = (q: CardQueryUi) =>
  req<GeneratedDescription>('describe_card', { ...q })

export const installPlugin = (body: {
  id: string
  scope: string
  cwd?: string
  acceptCommand?: string
}) => req<InstallResult>('install_plugin', { ...body })

// ─────────────────────────── external index ───────────────────────────

export const getRemote = (q: string, sort: string, type: string, limit = 30) =>
  req<RemoteCatalog>('get_remote', { q, sort, type, limit })

/** The MCP registry: search and read only — there is no install route next to it. */
export const getMcpRegistry = (q: string, limit = 30, cursor?: string) =>
  req<McpRegistry>('get_mcp_registry', { q, limit, cursor: cursor ?? null })

export const planRemoteInstall = (body: {
  repo: string
  name: string
  skillCommitSha?: string
  scope: string
  cwd?: string
}) => req<RemotePlan>('plan_remote_install', { ...body })

export const runRemoteInstall = (body: {
  repo: string
  name: string
  skillCommitSha?: string
  scope: string
  cwd?: string
  acceptCommand?: string
}) => req<RemoteInstallResult>('run_remote_install', { ...body })

// ─────────────────────────── statistics ───────────────────────────

/**
 * Slow on the first call — it walks every transcript on the machine — so only
 * the statistics window asks for it, on open.
 */
export const getStats = () => req<Stats>('get_stats')

/**
 * Drops every transcript older than `days` — the rule the page quoted when the
 * button was pressed, sent up with the request so the two cannot drift apart.
 */
export const cleanHistory = (days: number) =>
  req<{ deleted: number; bytes: number; failed: number }>('clean_history', { days })

// ─────────────────────────── optional modules ───────────────────────────

/**
 * The catalog: every module this build carries. The desktop build carries
 * none — openkb was dropped — so the list comes back empty.
 */
export const getModules = (probe = false) =>
  req<{ modules: ModuleRow[] }>('get_modules', { probe })

export const setModuleEnabled = (id: string, enabled: boolean) =>
  req<{ modules: ModuleRow[] }>('set_module_enabled', { id, enabled })

/**
 * A module's own routes, addressed by id. Nothing here knows what a module
 * does — only that a module owns the namespace under its id.
 */
export const modGet = <T,>(id: string, path: string, q?: Record<string, string>) =>
  req<T>('mod_get', { id, path, q: q ?? null })

export const modPost = <T,>(id: string, path: string, body?: unknown) =>
  req<T>('mod_post', { id, path, body: body ?? null })
