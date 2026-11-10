import type {
  Entity,
  PluginEntity,
  Profile,
  RuleState,
  Scope,
  SkillState,
} from '../shared/types.js'
import { skillStateOn } from '../shared/skill-state.js'

/**
 * Profile mutations that both the inventory rows and the ⌘K palette perform.
 * They live here so the two cannot drift apart: an entity toggled from the
 * palette must land in the draft exactly as a click on its row would.
 */

/**
 * Whether an entity is on in this profile. "all" means every entity on the machine,
 * which the listed names alone cannot say — reading the list alone made every row
 * look off while the profile had everything on.
 *
 * "all" is the profile saying nothing about MCP, so the files' own deny list
 * decides — a server named there does not start, whatever the mode says. That is
 * `offByFile`, and without it the box ticked a server the file refuses to load.
 */
export function isMcpOn(p: Profile, name: string, offByFile = false): boolean {
  if (p.mcp.mode === 'all') return !offByFile
  return p.mcp.mode === 'only' && p.mcp.servers.includes(name)
}

/**
 * An override map says nothing about a name it does not list — and "on" is the
 * absence of a key, so the map is never written with it. Two of the four states
 * still leave the skill within reach: "name-only" keeps it listed for the model,
 * "user-invocable-only" leaves it to a typed `/name`. Only the other two count as off.
 *
 * A name the map does not list falls to the file's own `skillOverrides`, which is
 * the same map one level down: this is why `offByFile` is a default rather than a
 * third state. The profile's opinion, when it has one, beats the file's.
 */
export function statesOn(p: Profile, name: string, offByFile = false): boolean {
  const s = p.skills.states[name]
  return s === undefined ? !offByFile : skillStateOn(s)
}

/**
 * Whether a skill is on. The all-or-nothing switch sits above the map: "none"
 * beats every state. A settings file cannot express it — see resolveProject.
 */
export function isSkillOn(p: Profile, name: string, offByFile = false): boolean {
  return p.skills.mode !== 'none' && statesOn(p, name, offByFile)
}

/**
 * A plugin has no mode — only the two lists. A plugin named in neither list is
 * one the profile has said nothing about, so it stays as it is on disk.
 */
export function isPluginOn(p: Profile, id: string, onDisk: boolean): boolean {
  if (p.plugins.disabled.includes(id)) return false
  if (p.plugins.enabled.includes(id)) return true
  return onDisk
}

/** Plugins that will not be there. A skill inside one of these is gone with it. */
export function offPluginIds(p: Profile, plugins: PluginEntity[]): Set<string> {
  return new Set(plugins.filter((pl) => !isPluginOn(p, pl.id, pl.enabled)).map((pl) => pl.id))
}

/**
 * Whether an entity will be in the session — the one question the left rows and
 * the right summary both ask. Two panes counting the same profile differently is
 * worse than either being wrong: it is what made "все (9)" sit next to nine
 * struck-through rows.
 */
export function isEntityLive(
  p: Profile,
  e: Entity,
  offPlugins: Set<string> = new Set(),
): boolean {
  switch (e.kind) {
    case 'mcp':
      return isMcpOn(p, e.name, e.offByFile)
    case 'plugin':
      return isPluginOn(p, e.id, e.enabled)
    case 'skill':
      // A plugin skill has one switch only — its own state map is not read for it.
      return e.plugin
        ? p.skills.mode !== 'none' && !offPlugins.has(e.plugin)
        : isSkillOn(p, e.name, e.offByFile)
    case 'hook':
      return !p.hooks.disableAll
    case 'memory':
      return p.memory.autoDiscover
  }
}

/**
 * The `skillOverrides` keys that decide nothing.
 *
 * Claude Code applies that map to skills only — "Plugin skills are not affected
 * by `skillOverrides`", says its documentation, and points at `/plugin` for
 * those. A plugin's skill therefore has one switch, its plugin, and an entry
 * written for it is a line that keeps being written into the file while changing
 * nothing. Clicks did write them (this app used to offer a box per skill row),
 * so they are not hypothetical: they sit in drafts and get applied.
 *
 * A name that also belongs to a skill of the project's own is kept — there the
 * entry is real, and the map is keyed by name alone.
 */
export function deadSkillStates(p: Profile, entities: Entity[]): string[] {
  const fromPlugin = new Set(
    entities.flatMap((e) => (e.kind === 'skill' && e.plugin ? [e.name] : [])),
  )
  const own = new Set(entities.flatMap((e) => (e.kind === 'skill' && !e.plugin ? [e.name] : [])))
  return Object.keys(p.skills.states).filter((n) => fromPlugin.has(n) && !own.has(n))
}

/**
 * Turning one entity off in "all" mode has no flag of its own, so the mode falls
 * back to an explicit list — and back to "all"/"none" when the list covers
 * everything or nothing. `all` is the inventory's names: the profile cannot know them.
 *
 * The set is built from the boxes as they are drawn, not from the mode: a server
 * the files deny sits unticked under "all", and starting from every name would
 * have ticked it. `denied` is that list, and a name only leaves it by being named
 * in `servers` — which is what takes the deny out of the file when this is applied.
 */
export function toggleMcp(
  p: Profile,
  name: string,
  all: string[],
  denied: string[] = [],
): Partial<Profile> {
  const base = p.mcp.mode === 'all' ? all : p.mcp.mode === 'only' ? p.mcp.servers : []
  const on = new Set(base.filter((n) => !denied.includes(n)))
  if (!on.delete(name)) on.add(name)
  const servers = all.filter((n) => on.has(n))
  // "all" only when the list really covers everything — a deny left in the files
  // keeps the profile explicit, because "all" would not lift it.
  const mode =
    servers.length === all.length && denied.length === 0
      ? 'all'
      : servers.length === 0
        ? 'disable'
        : 'only'
  return { mcp: { ...p.mcp, mode, servers } }
}

/**
 * The two lists are a delta from the disk, not a copy of it, so ticking a box
 * that already matches the disk has to leave no trace behind.
 *
 * It used to leave one: off-then-on moved the id out of `disabled` and into
 * `enabled`, and the build then wrote `"id": true` into `enabledPlugins` — a
 * change the user had not made, reported against a plugin the file already had
 * on. `onDisk` is what turns that round trip back into an identity. Without it
 * (no inventory yet) the profile has no way to know, so it states its intent.
 */
export function togglePlugin(
  p: Profile,
  id: string,
  enabled: boolean,
  onDisk?: boolean,
): Partial<Profile> {
  const on = new Set(p.plugins.enabled)
  const off = new Set(p.plugins.disabled)
  on.delete(id)
  off.delete(id)
  // Equal to the disk state means "no opinion", which is the absence of a key.
  if (enabled !== onDisk) {
    if (enabled) on.add(id)
    else off.add(id)
  }
  return { plugins: { enabled: [...on], disabled: [...off] } }
}

/** The rule a deny entry needs: `mcp__<server>__<tool>`, the server name verbatim. */
export function toolRule(server: string, tool: string): string {
  return `mcp__${server}__${tool}`
}

export function setSkillState(
  p: Profile,
  name: string,
  state: SkillState,
  offByFile = false,
): Partial<Profile> {
  const states = { ...p.skills.states }
  // "on" is written by deleting the key — an override map full of "on" is noise
  // in the settings file and in every diff of it. Except when a file turns the
  // skill off: there "on" is the whole point of the entry, and dropping the key
  // would leave the file's own word standing.
  if (state === 'on' && !offByFile) delete states[name]
  else states[name] = state
  return { skills: { ...p.skills, states } }
}

/**
 * One checkbox, four states behind it: off ↔ on. Turning a skill on inside a
 * profile where everything is off needs the mode back — "--disable-slash-commands"
 * beats the map, so a state written under it would do nothing.
 *
 * An off skill has two possible causes and they need opposite writes: the
 * profile's own "off" is undone by dropping the key, while a file's `off` can
 * only be beaten by naming the skill "on" — nothing else in the profile says it.
 */
export function toggleSkill(p: Profile, name: string, offByFile = false): Partial<Profile> {
  if (isSkillOn(p, name, offByFile)) return setSkillState(p, name, 'off', offByFile)
  const states = { ...p.skills.states }
  if (offByFile) states[name] = 'on'
  else delete states[name]
  return { skills: { mode: p.skills.mode === 'none' ? 'all' : p.skills.mode, states } }
}

/**
 * «Разрешить» is a decision like the other two and is written down like them: it
 * means the files should say nothing about this operation, which for a rule that
 * is already there means taking it out. It just never becomes an `allow` entry —
 * that is `withPermissions`'s business, not the profile's.
 *
 * A row may hold several rules — `Read` and `Edit` for one path — and they move
 * together: the row is one decision.
 */
export function setRules(p: Profile, rules: string[], state: RuleState): Partial<Profile> {
  const next = { ...p.permissions.rules }
  for (const rule of rules) {
    next[rule] = state
  }
  return { permissions: { rules: next } }
}
