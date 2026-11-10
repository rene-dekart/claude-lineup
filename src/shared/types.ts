/** Shared server and UI types. See docs/superpowers/specs/2026-10-05-*.md */

export type Scope = 'user' | 'project' | 'local' | 'plugin'

/** Provenance: where the entity came from. */
export interface Provenance {
  file: string
  line?: number
}

/**
 * One place a file switches an entity off: the file, and the key inside it that
 * names the entity. `file` comes already shortened for a row (`~/.claude.json`,
 * `.claude/settings.local.json`), so it is for reading, not for opening.
 */
export interface OffRef {
  file: string
  key: string
}

/**
 * A settings file as it lies on disk, for the read-only view: the path as a
 * heading, the parsed file as the text a person reads. Nothing here is a draft
 * and nothing here is written — this is what the machine holds right now.
 */
export interface ConfigFile {
  path: string
  content: string
}

/**
 * What a settings file can say about one permission rule. The three lists
 * resolve deny → ask → allow, first match wins, so a deny anywhere beats an
 * allow anywhere else.
 */
export type RuleState = 'allow' | 'ask' | 'deny'

/**
 * What a profile asks for. All three are real decisions, `allow` included — but
 * `allow` is never written as an entry. It means "no rule at all", so it strips
 * whatever the files say about the operation and adds nothing, because an `allow`
 * entry is the weakest of the three lists and any `deny` elsewhere would overrule
 * it anyway. See `withPermissions`.
 */
export type RuleOpinion = RuleState

/**
 * One operation the user decides about — every row comes from the shipped
 * catalog, and what makes it a report of this machine is `state`: the same row
 * reads differently depending on what the files already say.
 */
export interface PermissionRow {
  /** Russian name of the operation. */
  name: string
  /** The command or path it is about — shown muted, next to the name. */
  cmd: string
  /** One sentence: what the operation does. */
  why: string
  /** The rules this row writes. One row may need two — `Read` and `Edit`. */
  rules: string[]
  /** What the files say today. `null` — none of them mentions the rule, so it
   *  runs under `defaultMode` and no line is written. */
  state: RuleState | null
  /** Where that state was found. Absent when no file mentions the rule. */
  scope?: Scope
  /** Which heading it reads under. */
  group: string
}

/**
 * Where a figure came from. The level says how good it is, this says who
 * produced it — and the two are not the same question: Claude Code's own count
 * and our live probe are both `measured`, but only one of them is a fact about
 * a session that really ran.
 */
export type CostSource = 'session' | 'probe' | 'size'

/**
 * Cost in context. An exact number cannot be given, so the honesty
 * level is mandatory: measured — we measured it, estimated — we guessed,
 * unknown — we don't know and don't lie.
 */
export interface Cost {
  level: 'measured' | 'estimated' | 'unknown'
  tokens?: number
  note: string
  source?: CostSource
}

export type Entity =
  | McpEntity
  | PluginEntity
  | SkillEntity
  | HookEntity
  | MemoryEntity

export interface McpEntity {
  kind: 'mcp'
  name: string
  scope: Scope
  transport: 'stdio' | 'http'
  command?: string
  url?: string
  source: Provenance
  enabled: boolean
  /**
   * A settings file refuses to load this server: `deniedMcpServers`, or the
   * project's own `disabledMcpServers`. That list merges from every file and
   * beats anything that says "on", so a profile silent about MCP cannot turn
   * such a server on — only naming it can, which is what the toggle writes.
   * Without this the row claims a server is on that never starts.
   */
  offByFile?: boolean
  /**
   * The same fact with names on it: which file, and which key in that file,
   * switches this server off for this project. The checkbox above can only say
   * "off" — and for a project the answer to "where" is the whole question, since
   * the switch sits in a file the user never opens.
   */
  offBy?: OffRef[]
  /**
   * What the server is busy with. MCP has no static description at all, so
   * the source is only one of two: the description of the plugin that pulled
   * it in, or tool names from a live probe. No source — no line.
   */
  description?: string
  cost: Cost
}

export interface PluginEntity {
  kind: 'plugin'
  /** format "name@marketplace" */
  id: string
  scope: Scope
  source: Provenance
  enabled: boolean
  /** How many skills the plugin pulls in. */
  skillCount: number
  description?: string
  cost: Cost
}

export interface SkillEntity {
  kind: 'skill'
  name: string
  origin: 'user' | 'plugin' | 'project'
  plugin?: string
  /** Whether the description enters context (affects disable-model-invocation). */
  modelInvocable: boolean
  /**
   * A settings file switches this skill off: `skillOverrides` holds `off` or
   * `user-invocable-only` for it. The map says nothing about a name it does not
   * list, so its absence is not an opinion — the profile's own state overrides
   * this, and a profile that says nothing leaves the file's word standing.
   * Plugin skills are exempt: skillOverrides does not apply to them.
   */
  offByFile?: boolean
  /** frontmatter description — exactly the text that goes into context. */
  description?: string
  source: Provenance
  cost: Cost
}

export interface HookEntity {
  kind: 'hook'
  event: string
  matcher: string
  command: string
  scope: Scope
  source: Provenance
  cost: Cost
}

export interface MemoryEntity {
  kind: 'memory'
  path: string
  source: Provenance
  cost: Cost
}

/**
 * Claude Code's own `/context` report, read off the disk. Its numbers are
 * counts of what the harness really sent — better than any estimate we can
 * derive. What it cannot answer is "what will THIS profile cost": it is a
 * measurement of one session that already ran.
 */
export interface SessionReport {
  /** When that session ran, ISO. */
  at: string
  /** The transcript the report was read from. */
  file: string
  /** Total the harness reported, if the line was there. */
  total?: number
  /**
   * The denominator of the same line — `41.6k/200k`. The only honest scale a bar
   * can be drawn against: our own estimate has no ceiling otherwise, and the
   * ceiling is what makes a share mean anything.
   */
  window?: number
  /**
   * The one part of the budget the profile cannot choose: system prompt,
   * system tools, custom agents. Background that is always paid.
   */
  base: number
  /** Category → tokens, exactly as printed. */
  categories: Record<string, number>
  /** MCP server → tokens (server names, plugin servers as `plugin_x_y`). */
  mcp: Record<string, number>
  /** Memory file → tokens, keyed by absolute path (see `memoryKey`). */
  memory: Record<string, number>
}

export interface Inventory {
  entities: Entity[]
  /** Permission rows: the shipped catalog plus whatever the files already say. */
  permissions: PermissionRow[]
  /**
   * Every rule the files already carry, whether or not the catalog has a row for
   * it. The rows above can only hold catalog rules, and the card's per-tool boxes
   * need the rest — a server's tools are denied in `permissions.deny` and nowhere
   * else, so without this map those boxes were drawn from the profile alone and
   * stayed ticked over a file that denies them.
   */
  fileRules: Record<string, RuleState>
  /** Files that could not be read — we don't fail the inventory because of them. */
  unreadable: Array<{ file: string; error: string }>
  /**
   * Config problems found: keys that Claude Code silently
   * ignores. They are visible only by eye in the code, so we collect them here.
   */
  notes: string[]
  /** Collection moment, ISO. */
  scannedAt: string
  /** The session's own numbers, when a `/context` report was found. */
  report?: SessionReport
}

// ─────────────────────────── catalog ───────────────────────────

/**
 * A plugin the machine knows about but that is not necessarily installed.
 * Sources: the Anthropic catalog cache (it has descriptions, install counts
 * and tokens computed by THEM) and marketplace manifests on disk.
 */
export interface CatalogEntry {
  /** format "name@marketplace" */
  id: string
  name: string
  marketplace: string
  description: string
  category?: string
  version?: string
  /** How many times the plugin was installed — the only usefulness signal in the catalog. */
  installs?: number
  bundledSkills: string[]
  bundledMcp: string[]
  /** Anthropic catalog numbers, not our measurement. Level estimated. */
  cost: Cost
  installed: boolean
  /** GitHub repo off the marketplace manifest. Absent when the manifest has none. */
  link?: string
}

export interface Catalog {
  entries: CatalogEntry[]
  /** How many matched the query in total — so the UI can say "showing 100 of N". */
  total: number
  /** Where this page starts — the UI pages through `total` with it. */
  offset: number
  marketplaces: Array<{ name: string; plugins: number; source: string }>
  notes: string[]
  scannedAt: string
}

/**
 * What the detail card shows. One shape for all five kinds: the fields a kind
 * does not have are absent, and the UI renders what is there.
 */
export interface ToolInfo {
  name: string
  /** Tokens this tool's own JSON contributes to tools/list. */
  tokens: number
  /** The tool's own `description`, first line only. */
  desc?: string
}

export interface Card {
  kind: Entity['kind']
  title: string
  description?: string
  /** Full text of the skill's SKILL.md / the memory file, frontmatter stripped. */
  body?: string
  file?: string
  scope?: string
  origin?: string
  command?: string
  url?: string
  /** GitHub repo, read off the disk. No source on disk — no link, never guessed. */
  link?: string
  transport?: string
  modelInvocable?: boolean
  /**
   * One tool of an MCP server, from the last live probe. The figure is that
   * tool's own JSON — name, description, input schema — which is exactly what
   * the model is handed. Optional: a cache entry written before per-tool
   * measurement existed holds bare names, and a zero there would be a lie.
   */
  tools?: ToolInfo[]
  marketplace?: string
  category?: string
  version?: string
  installs?: number
  bundledSkills: string[]
  bundledMcp: string[]
  /** Plugins that pulled this MCP server in — for MCP this is the only description source. */
  owners: string[]
  cost?: Cost
  notes: string[]
}

/**
 * A description WE wrote, because the author wrote none. The card shows it the
 * same way it shows the author's text — one line of Russian, no provenance note.
 */
export interface GeneratedDescription {
  /** Absent when there was nothing to work from, or the model failed. */
  text?: string
  /** Why it did not work: no key, no network, no source material. */
  error?: string
}

/**
 * The OS picker, for a folder or a file. `path: null` means the user closed it —
 * that is not an error, and `error` is what the picker itself failed on.
 */
export interface PickResult {
  path: string | null
  error?: string
}

/**
 * Plugin install result: the CLI installs, our code does not write to ~/.claude.
 */
export interface InstallResult {
  ok: boolean
  code: number | null
  stdout: string
  stderr: string
  /**
   * The marketplace declared installation via a command. Show it to the
   * human and ask again — a foreign command cannot be run silently.
   */
  pending?: { command: string; sha256: string }
}

// ─────────────────────── network: external index ───────────────────────

/**
 * An external index entry (skillselion). This is NOT a marketplace and not a plugin:
 * the index knows about the repository, but you cannot install from there — installation
 * still goes through `marketplace add <repo>`. Fields are named as in their API,
 * so we don't invent our own vocabulary layer.
 */
export interface RemoteListing {
  id: string
  name: string
  /** skill | plugin | marketplace | mcp — their vocabulary. */
  type: string
  author: string
  /** owner/repo — the only thing that turns into an action. */
  repo: string
  description: string
  summary?: string
  category?: string
  stars?: number
  installs?: number
  votes?: number
  trustScore?: number
  verifiedSafe: boolean
  riskLevel?: string
  qualityTier?: string
  requiresApiKeys?: boolean
  costToRun?: string
  /** Commit SHA where the skill lives: the only available version pin. */
  skillCommitSha?: string
  bestFor?: string
}

export interface RemoteCatalog {
  entries: RemoteListing[]
  /** Total from stats, not the page length — their API does not return total per query. */
  total: number
  hasMore: boolean
  /** Host the entries came from. The window names the source; env can move it. */
  source: string
  notes: string[]
  fetchedAt: string
}

/**
 * One server from the official MCP registry. Unlike the plugin index this one
 * knows how to connect — `remotes` (a running server's url) or `packages` (a
 * local one) — and carries no quality signal at all: the registry stores
 * metadata and deliberately no stars, no install counts, no ratings. So there
 * is nothing here to rank by, and the repository is the only thing that speaks
 * for a server. That is where the row sends you.
 */
export interface McpRegistryRow {
  /** Reverse-DNS id: `io.github.user/server`. The registry's own identity for it. */
  name: string
  title: string
  description: string
  version: string
  /** `owner/repo`, when the author declared a repository. */
  repo?: string
  /** How it connects: `streamable-http`, `sse`, or `stdio` for a local package. */
  transports: string[]
  /** Connecting takes a secret the author did not ship — a token or a key. */
  secret?: boolean
}

/** One page of registry search results, plus the cursor for the next one. */
export interface McpRegistry {
  entries: McpRegistryRow[]
  /** Cursor for the page after this one. Absent means this was the last. */
  nextCursor?: string
  /** Host the entries came from. The window names the source; env can move it. */
  source: string
  notes: string[]
  fetchedAt: string
}

/** One step of the install chain: plan and actual result look the same. */
export interface InstallStep {
  /** Ready-made line to show the human. */
  display: string
  argv: string[]
  ok?: boolean
  code?: number | null
  stdout?: string
  stderr?: string
}

export interface RemotePlan {
  ok: boolean
  repo: string
  steps: InstallStep[]
  marketplaceName?: string
  pluginId?: string
  /** Marketplace already added — the first step is not needed. */
  alreadyAdded: boolean
  /** No version pin set: the repository will be cloned at the default branch HEAD. */
  unpinned: boolean
  notes: string[]
  error?: string
}

export interface RemoteInstallResult {
  ok: boolean
  steps: InstallStep[]
  error?: string
  pending?: { command: string; sha256: string; step: number }
}

// ─────────────────────────── profiles ───────────────────────────

/** All-or-nothing switch: Claude Code has no "only these" mode. */
export type SkillMode = 'all' | 'none'
/** What `skillOverrides` accepts per skill. A name absent from the map is "on". */
export type SkillState = 'on' | 'name-only' | 'user-invocable-only' | 'off'
export type McpMode = 'all' | 'only' | 'disable'

export interface Profile {
  id: string
  title: string

  /** `rules` are whole permission rules keyed by the state the profile wants
   *  them in. A rule absent from the map is one the profile says nothing about,
   *  and the files are left as they are. */
  permissions: { rules: Record<string, RuleOpinion> }
  mcp: { mode: McpMode; servers: string[] }
  plugins: { enabled: string[]; disabled: string[] }
  skills: { mode: SkillMode; states: Record<string, SkillState> }
  hooks: { disableAll: boolean }
  memory: { autoDiscover: boolean }
  flags: string[]

  /**
   * Emergency hatch: a raw settings.json patch. Unfamiliar keys pass
   * through and are never lost when the Claude Code schema changes.
   */
  claude: Record<string, unknown>
}

export interface FileChange {
  path: string
  before: string | null
  after: string | null
}

/**
 * What one apply left behind, kept so it can be undone. One apply = one point:
 * the unit of undo is the whole write, because a file restored on its own
 * leaves a project half-turned-back, which is a state nobody asked for.
 */
export interface RestorePoint {
  id: string
  /** Epoch ms of the write the point belongs to. */
  ts: number
  /** The project it was applied to: shown in the history, and the bound a
   *  restore is not allowed to write outside of. */
  cwd: string
  /** `'profile'` is the app's own profile file: the save kept the version it
   *  replaced, and that is a point like any other — same history, same button. */
  mode: 'project' | 'profile'
  files: Array<{ path: string; existed: boolean }>
}

export interface Resolution {
  changes: FileChange[]
  warnings: string[]
}

// ─────────────────────────── statistics ───────────────────────────

/**
 * Tokens a session was billed for, as the API counted them. Four separate
 * numbers on purpose: a cache read costs a fraction of an input token, so
 * adding them into one figure would overstate the bill by an order of magnitude.
 */
export interface TokenCounts {
  input: number
  output: number
  /** Context re-sent from the cache — cheap, but real. */
  cacheRead: number
  /** Context written into the cache. */
  cacheCreate: number
  /** Reasoning tokens. Already inside `output` — shown apart, never added. */
  thinking: number
}

/**
 * One session, as the API billed it. The unit the panel cuts in every direction:
 * totals, days, projects and models are all folds over a list of these, so any
 * filter — one project, one model, one day, or the three combined — is the same
 * arithmetic over a shorter list, and the figures always agree with each other.
 */
/**
 * What a session asked for, by name: every MCP server it called, every skill it
 * invoked. Names are the ones Claude Code wrote into the transcript — `github`,
 * `superpowers:brainstorming`, `plugin_context-mode_context-mode` — so the plugin
 * behind a name is read off the name itself. The inventory is not consulted: it
 * describes today's disk, these are calls that already happened, and the two are
 * allowed to disagree (a plugin uninstalled since still made those calls).
 *
 * The tool inside a server is deliberately dropped. `mcp__plugin_context-mode_
 * context-mode__ctx_search` is 45 characters of a name against a decision — does
 * this server earn its place — that is taken per server.
 */
export interface Usage {
  /** MCP server name → calls. */
  mcp: Record<string, number>
  /** Skill name as invoked → calls. A plugin's skill carries its plugin as a prefix. */
  skill: Record<string, number>
}

export interface SessionStat {
  id: string
  /** The cwd the session ran in. `''` when no line carried one. */
  project: string
  /** First turn, epoch ms. Zero when no line carried a usable timestamp. */
  start: number
  /** Last turn, epoch ms. */
  end: number
  /** Assistant turns that were billed. */
  messages: number
  /**
   * Model → that model's own four columns, within this session. Full counts and
   * not just the output: `usage` is reported per API call and each call names the
   * model that answered it, so a model can be told apart from the session it sat
   * in. Keying this on a bare output number would leave the input and cache tiles
   * reporting the whole session under a filter that names one model.
   */
  models: Record<string, TokenCounts>
  tokens: TokenCounts
  /**
   * What the session called, and how many times. Counted in the same pass as the
   * tokens, off the same assistant lines — a turn that calls a tool is a billed
   * turn, so nothing has to be read twice.
   */
  usage: Usage
  /**
   * The longest unbroken stretch of work in this session, ms. A gap longer than
   * half an hour ends a stretch: a session resumed two days later is not a
   * two-day session, and measuring first turn to last would call it one.
   */
  longestMs: number
}

/** A project as the panel shows it — a fold over `SessionStat[]`, not a file. */
export interface ProjectStat {
  /** The cwd the sessions ran in. */
  path: string
  sessions: number
  messages: number
  tokens: TokenCounts
  /** Last turn, epoch ms. */
  last: number
}

/** One day of the window the bars are drawn from. */
export interface DayStat {
  /** YYYY-MM-DD, local time. */
  date: string
  sessions: number
  messages: number
  /**
   * The day's tokens at full rate — input and output, kept apart. Deliberately
   * NOT the sum of all four columns: a cache read is a fraction of an input
   * token, so folding it in would make the bar and the figure under it overstate
   * the day by an order of magnitude. Cache traffic is shown where it belongs,
   * in the tiles.
   */
  input: number
  output: number
}

/**
 * What one saving tool reports about itself. rtk and context-mode count their
 * own bytes, so their figures are measurements, not ours. A tool that is not
 * installed, or that has written nothing, says so — a zero would read as
 * "this tool saves nothing", which is a different claim.
 */
export interface SavingsSource {
  /** Stable id, also the i18n key suffix: 'rtk' | 'context-mode' | 'caveman'. */
  id: string
  /** Tokens saved. Absent when unknown. */
  saved?: number
  /** Tokens that went in, for scale. Absent when the tool does not report it. */
  total?: number
  /** Commands or calls it acted on. */
  calls?: number
  /** Its own note: when the data was written, or why there is none. */
  note?: string
}

/**
 * The transcripts as a pile of bytes.
 *
 * Claude Code prunes `~/.claude/projects` itself, at startup, older than its own
 * `cleanupPeriodDays`. So the oldest session the panel can see is usually the
 * retention boundary and not the day the user installed the tool — a fact about
 * the disk, which the page states rather than leaves to be guessed.
 */
export interface HistorySize {
  files: number
  bytes: number
  /** Oldest transcript mtime, epoch ms. Zero when there are none. */
  oldest: number
  /** What the rule would drop today: files past `retentionDays`. */
  oldFiles: number
  oldBytes: number
  /** The rule in days — Claude Code's own setting, or its default. */
  retentionDays: number
}

export interface Stats {
  /**
   * Every session on the machine, one record each, unaggregated. The panel folds
   * this list itself rather than asking the server for each cut: one fact table,
   * sliced on the client, so a filter recomputes instantly and from the same
   * numbers the unfiltered view was drawn from. A couple of hundred records here;
   * over localhost the payload is free.
   * ponytail: this does not bound. At ~10k sessions it is a ~2 MB response, and
   * the fold moves back to the server with a query parameter when that happens.
   */
  sessions: SessionStat[]
  savings: SavingsSource[]
  /** The pile those sessions were read out of. */
  history: HistorySize
  /** What could not be read. Never a silent zero. */
  notes: string[]
  /** Collection moment, ISO. */
  scannedAt: string
}

/* ---------- Optional modules ---------- */

/**
 * Is the tool on this machine? Two answers and no third one: the probe starts a
 * process, and a process either runs or does not. `unknown` would be a guess
 * wearing a fact's clothes.
 */
/**
 * `unknown` is the honest third answer, as `level` is for a cost: a probe starts a
 * process, and the app does not start one until a human opens the catalog or the
 * module's own page. Nobody asked yet — that is not the same as "not found".
 */
export type ModuleProbeState = 'found' | 'not-found' | 'unknown'

export interface ModuleProbe {
  state: ModuleProbeState
  /** Version, or the knowledge base's root — whatever the probe managed to read. */
  detail?: string
  version?: string
}

/** One row of the catalog: the module, whether the tool is here, and its switch. */
export interface ModuleRow {
  id: string
  /** i18n key of the display name. The core never carries the name itself. */
  titleKey: string
  enabled: boolean
  probe: ModuleProbe
}

/* ---------- Knowledge base (the openkb module) ---------- */

/** A document in `raw/` — what was added, before the wiki compiled it. */
export interface KbDoc {
  name: string
  bytes: number
  mtime: number
  /** Pages linking to its summary. */
  links: number
}

/** A wiki page as it appears in a list: weighed by links, never by name alone. */
export interface KbPage {
  /** Path inside the KB root — the id the UI page view asks back with. */
  path: string
  bytes: number
  mtime: number
  in: number
  out: number
}

export interface KbCounts {
  raw: number
  wiki: number
  sources: number
  summaries: number
  explorations: number
}

export interface KbTree {
  /** Null when openkb is here but no knowledge base is named. Not an error. */
  root: string | null
  docs: KbDoc[]
  entities: KbPage[]
  concepts: KbPage[]
  counts: KbCounts
}

/** The three lists the UI switches between; a slice is asked for by name. */
export type KbGroup = 'docs' | 'entities' | 'concepts'

/** A row of a list: a document is a file, the other two are pages the UI draws the same way. */
export type KbRow = KbDoc | KbPage

/**
 * One slice of one list, and how long the whole list is.
 *
 * `total` exists because `rows.length` stops being the size of anything the moment a
 * slice can be short: a hundred rows out of a hundred and a hundred out of nine
 * hundred look identical without it.
 */
export interface KbList {
  /** Null when openkb is here but no knowledge base is named. Not an error. */
  root: string | null
  counts: KbCounts
  group: KbGroup
  offset: number
  rows: KbRow[]
  total: number
}

export interface KbPageView {
  path: string
  bytes: number
  mtime: number
  text: string
  /** Slugs this page links out to. */
  out: string[]
  /** Paths of pages linking in. */
  in: string[]
  /**
   * Where this page's `[[links]]` land: the target as written → a path inside the KB,
   * or null when nothing in the wiki answers to that name. A dead link stays in the
   * text — it is a fact about the wiki, not a failure to render.
   */
  links: Record<string, string | null>
}

export interface KbHit {
  path: string
  line: number
  text: string
}

/** What `openkb add` said. `code` is the tool's own exit code. */
export interface KbAddResult {
  code: number
  stdout: string
  stderr: string
}
