import type { Cost, Entity, Inventory, Profile } from '../shared/types.js'
import { isMcpOn, isPluginOn, statesOn } from './profile-ops.js'

/**
 * What the profile actually puts in context, in tokens.
 *
 * Not the sum of the inventory: the inventory is what exists on the machine,
 * this is what the launch will load. The two differ exactly where the form lies
 * — a plugin's skill row stays counted while the plugin itself is off, and the
 * all-or-nothing skill switch above the rows is not a thing a file can write.
 * Mirrors what resolveProject really does, so the number means something.
 *
 * The price: rows with no known size (every MCP server nobody probed) are
 * counted in `all` but contribute nothing to `tokens`. Hence the "known of all".
 */
export interface ContextTotal {
  /** Absent when nothing selected has a size — then there is no number to show. */
  tokens?: number
  level: Cost['level']
  known: number
  all: number
  /** Background no profile can choose (system prompt, tools, agents), from the report. */
  base: number
  /** When the session that produced `base` ran, ISO. */
  reportAt?: string
  /** The same count with everything in the inventory switched on — the reference. */
  full?: number
  /** How much this profile saves against it. The number a configurator exists for. */
  delta?: number
}

export function contextTotal(inv: Inventory, p: Profile): ContextTotal {
  const onDisk = new Map(
    inv.entities.filter((e) => e.kind === 'plugin').map((e) => [e.id, e.enabled]),
  )
  // The profile writes files: it can switch plugins and all hooks off, nothing
  // else — the same four warnings resolveProject prints.
  const hooksOn = !p.hooks.disableAll

  /**
   * A plugin row's size is the sum of its skills' descriptions — the same text
   * the skill rows below carry. Counting both would count it twice, and the
   * plugin contributes nothing of its own (its hooks cost no context). So a
   * plugin whose skills we can see is a label, not a number.
   */
  const enumerated = new Set(
    inv.entities.flatMap((e) => (e.kind === 'skill' && e.plugin ? [e.plugin] : [])),
  )
  const size = (e: Entity): number | undefined =>
    e.kind === 'plugin' && enumerated.has(e.id) ? undefined : e.cost.tokens

  const picked = inv.entities.filter((e) => {
    switch (e.kind) {
      case 'mcp':
        // Not every server the machine can see: one switched off in the file, and
        // one the file denies, does not start either.
        return isMcpOn(p, e.name, e.offByFile)
      case 'plugin':
        return isPluginOn(p, e.id, e.enabled)
      // A skill lives inside its plugin: plugin off, skill gone with it. And for a
      // plugin's skill the plugin is the only switch that counts — `skillOverrides`
      // does not apply to those (Claude Code's own words), so an entry left in the
      // map would have discounted tokens the launch still loads.
      case 'skill':
        return e.plugin
          ? isPluginOn(p, e.plugin, onDisk.get(e.plugin) ?? false)
          : statesOn(p, e.name, e.offByFile)
      case 'hook':
        return hooksOn
      // A settings file has no key for auto-memory: what is on the disk loads.
      case 'memory':
        return true
    }
  })

  const base = inv.report?.base ?? 0
  const known = picked.filter((e) => size(e) !== undefined)
  const own = known.reduce((s, e) => s + (size(e) ?? 0), 0)
  const tokens = own + base

  // The reference: every entity in the inventory on, plus the same background.
  // An upper bound, and shown as one — "everything" is not a profile anyone runs.
  const allKnown = inv.entities.filter((e) => size(e) !== undefined)
  const full = allKnown.reduce((s, e) => s + (size(e) ?? 0), 0) + base

  return {
    ...(tokens > 0 ? { tokens } : {}),
    level:
      known.length > 0 && known.every((e) => e.cost.level === 'measured') ? 'measured' : 'estimated',
    known: known.length,
    all: picked.length,
    base,
    ...(inv.report ? { reportAt: inv.report.at } : {}),
    ...(full > 0 ? { full } : {}),
    ...(full > tokens ? { delta: full - tokens } : {}),
  }
}
