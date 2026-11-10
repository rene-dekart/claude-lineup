import type { Profile, RuleOpinion, RuleState } from './types.js'

/**
 * The settings this profile asks for, built in one place because two consumers
 * read it: the server, which turns it into `--settings` or writes it into
 * `.claude/settings.json`, and the pane, which shows it as the file it becomes.
 * Two builders would drift, and the pane would then be showing a file that is
 * not the one that gets written.
 */

/** `enabledPlugins` is a map, and only the plugins the profile has an opinion about go in. */
function enabledPluginsPatch(p: Profile): Record<string, boolean> {
  const out: Record<string, boolean> = {}
  for (const id of p.plugins.disabled) out[id] = false
  for (const id of p.plugins.enabled) out[id] = true
  return out
}

/**
 * `allow`, `ask` and `deny` are lists, and a settings merge replaces lists
 * instead of extending them. So every rule already there — in the raw patch, in
 * the file on disk — goes back in alongside ours, or writing one rule would
 * quietly drop the others.
 *
 * A rule we are moving has to leave the list it came from: deny beats ask beats
 * allow, so a stale deny left behind would decide the question no matter what
 * the profile now says. That subtraction is the whole reason this is one
 * function and not three `[...new Set([...])]` calls.
 *
 * A rule the profile wants in `allow` goes into no list at all — it is subtracted
 * and then written nowhere. Writing it as an `allow` entry would put a line in the
 * user's file that cannot win anything, since a deny in any of the three files
 * outranks it.
 */
function withPermissions(
  permissions: unknown,
  alreadyThere: unknown,
  rules: Record<string, RuleOpinion>,
): Record<string, unknown> {
  const base = (permissions && typeof permissions === 'object' ? permissions : {}) as Record<
    string,
    unknown
  >
  const list = (src: unknown, key: RuleState): string[] => {
    const v = (src as Record<string, unknown> | undefined)?.[key]
    return Array.isArray(v) ? v.map(String) : []
  }

  const out: Record<string, unknown> = { ...base }
  for (const state of ['allow', 'ask', 'deny'] as RuleState[]) {
    // `allow` contributes nothing to any list: the subtraction below is the whole
    // of what that decision does.
    const mine =
      state === 'allow'
        ? []
        : Object.entries(rules)
            .filter(([, s]) => s === state)
            .map(([r]) => r)
    // Everything the files already had, minus the rules the profile now has an
    // opinion about — those go in `mine` if they belong here, and nowhere if not.
    const kept = [...list(permissions, state), ...list(alreadyThere, state)].filter(
      (r) => !(r in rules),
    )
    // Kept first, ours appended: a rule that was already in the file should not
    // jump to the top of the diff because a later decision moved it.
    const all = [...new Set([...kept, ...mine])]
    // An empty list is left out rather than written: `"ask": []` is a line in
    // the user's file that says nothing.
    if (all.length > 0) out[state] = all
    else delete out[state]
  }
  return out
}

/**
 * The two lists that actually switch an MCP server off, whichever file carries
 * them: `disabledMcpServers` in the project entry of `~/.claude.json`, and
 * `disabledMcpjsonServers` in `.claude/settings.local.json`. Both are plain names,
 * and neither is part of `settingsPatch` above — they live in files of their own,
 * and `resolveProject` writes each where its server was declared.
 *
 * `deniedMcpServers` is deliberately NOT one of them. It looks like the right key
 * and is not: Claude Code honors it only from managed (enterprise) settings, and
 * silently ignores it in a project file — which is what this app used to write.
 *
 * A merge replaces lists, so the names already in the file come back except for
 * the ones the profile now has an opinion about: the toggle decides, and a stale
 * name left behind would keep a server off that was just turned back on.
 * `known` is the universe the opinion is about — a name the files never declared
 * cannot be named here at all, and a name the profile says nothing about is not
 * ours to remove.
 */
export function disabledServersPatch(
  existing: unknown,
  off: string[],
  known: string[],
): string[] {
  const mine = new Set(known)
  const kept = (Array.isArray(existing) ? existing : []).filter(
    (n): n is string => typeof n === 'string' && !mine.has(n),
  )
  return [...new Set([...kept, ...off])]
}

/**
 * Where a name is declared decides which file has to carry its switch: a server
 * from `~/.claude.json` (user or local scope) is switched off in the project entry
 * of that same file, one from the project's `.mcp.json` in the project's local
 * settings. A name declared in both goes to the first — the file Claude Code reads
 * last wins the definition, and the switch that matters is the one it obeys.
 */
export function splitByOrigin(
  off: string[],
  origins: Record<string, 'user' | 'local' | 'project'>,
): { inClaudeJson: string[]; inProjectFile: string[] } {
  const inClaudeJson: string[] = []
  const inProjectFile: string[] = []
  for (const name of off) {
    if (origins[name] === 'project') inProjectFile.push(name)
    else inClaudeJson.push(name)
  }
  return { inClaudeJson, inProjectFile }
}

/**
 * `existingPermissions` is the `permissions` object already in the target file,
 * so a rule moved out of `deny` does not survive the per-key merge. The pane that
 * previews the patch alone passes nothing — it has no file to merge with.
 *
 * `mcpServers` is deleted rather than passed through: Claude Code ignores the key
 * in a settings file, so writing it would look like it worked. The caller warns
 * about the drop.
 */
export function settingsPatch(p: Profile, existingPermissions?: unknown): Record<string, unknown> {
  const out: Record<string, unknown> = { ...p.claude }
  const ep = enabledPluginsPatch(p)
  if (Object.keys(ep).length > 0) out['enabledPlugins'] = ep
  if (p.hooks.disableAll) out['disableAllHooks'] = true
  delete out['mcpServers']
  if (Object.keys(p.skills.states).length > 0) out['skillOverrides'] = p.skills.states
  if (Object.keys(p.permissions.rules).length > 0) {
    const perms = withPermissions(out['permissions'], existingPermissions, p.permissions.rules)
    // Nothing left means the profile wants no rule at all — and that is said by
    // the absence of the key, not by an empty object in the user's settings.
    if (Object.keys(perms).length > 0) out['permissions'] = perms
    else delete out['permissions']
  }
  return out
}
