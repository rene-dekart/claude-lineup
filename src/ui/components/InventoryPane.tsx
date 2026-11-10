import { useEffect, useMemo, useState, type KeyboardEvent } from 'react'
import { IconBolt } from './Icon.js'
import type {
  Cost,
  Entity,
  Inventory,
  PermissionRow,
  PluginEntity,
  Profile,
  RuleOpinion,
  RuleState,
  Scope,
} from '../../shared/types.js'
import { formatCost, sourceKey } from '../diff.js'
import { useT } from '../i18n.js'
import * as ops from '../profile-ops.js'

/**
 * The token figure and where it came from. Three sources look three different
 * ways — a bare number is Claude Code's own count from a real session, ≈ is our
 * live measurement, ~ is a file size — so a guess is never read as a fact.
 */
function Chip({ cost }: { cost: Cost }) {
  const t = useT()
  return (
    <span
      className={`chip ${cost.level} ${cost.source ?? ''}`}
      title={`${t(sourceKey(cost))} — ${cost.note}`}
    >
      {formatCost(cost)}
    </span>
  )
}

interface Props {
  inventory: Inventory | null
  profile: Profile
  busy: boolean
  onChange: (p: Partial<Profile>) => void
  onProbe: (name: string) => void
  /** Open the detail card — what this row actually is and what it does. */
  onOpen: (kind: Entity['kind'], name: string, file?: string) => void
  /** Move a permission row: the rules it writes, and the state wanted. */
  onRule: (rules: string[], state: RuleState) => void
}

/** Hooks have no group here — see byKind below. */
type ShownKind = Exclude<Entity['kind'], 'hook'>

const TITLES: Record<ShownKind, string> = {
  mcp: 'inv.kindMcp',
  plugin: 'inv.kindPlugin',
  skill: 'inv.kindSkill',
  memory: 'inv.kindMemory',
}

const ORDER: ShownKind[] = ['mcp', 'plugin', 'skill', 'memory']

/** The permission section's own key in the fold state — it has no entity kind. */
const PERM_KEY = 'permissions'

/** Headings inside the section, in reading order. */
const PERM_ORDER = ['files', 'repo', 'system', 'data'] as const

/** Deny beats ask beats allow, wherever the rule sits. */
const RANK: Record<RuleState, number> = { allow: 0, ask: 1, deny: 2 }

/**
 * What a permission row shows: the profile's own decision when it has one, and
 * what the files say otherwise. It has to be both — the files are the truth about
 * the machine, but they do not change when the user clicks, so reading them alone
 * would leave the control ignoring the click that was just made.
 *
 * A row can hold two rules (`Read` and `Edit`) and the profile may have an opinion
 * about one of them; the stronger of the two is what the row reads as, the same
 * way the scanner picks between them.
 */
function shownState(r: PermissionRow, rules: Record<string, RuleOpinion>): RuleState {
  let own: RuleState | null = null
  for (const rule of r.rules) {
    const s = rules[rule]
    if (s && (!own || RANK[s] > RANK[own])) own = s
  }
  return own ?? r.state ?? 'allow'
}

const PERM_TITLES: Record<string, string> = {
  files: 'inv.permFiles',
  repo: 'inv.permRepo',
  system: 'inv.permSystem',
  data: 'inv.permData',
}

const RULE_STATES: RuleState[] = ['allow', 'ask', 'deny']

const RULE_LABELS: Record<RuleState, string> = {
  allow: 'inv.ruleAllow',
  ask: 'inv.ruleAsk',
  deny: 'inv.ruleDeny',
}

const RULE_TITLES: Record<RuleState, string> = {
  allow: 'inv.ruleAllowTitle',
  ask: 'inv.ruleAskTitle',
  deny: 'inv.ruleDenyTitle',
}

/** A chip is one line by design; the whole sentence stays in the tooltip. */
const cut = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s)

export function InventoryPane({
  inventory,
  profile,
  busy,
  onChange,
  onProbe,
  onOpen,
  onRule,
}: Props) {
  const t = useT()

  const byKind = useMemo(() => {
    const m = new Map<Entity['kind'], Entity[]>()
    for (const e of inventory?.entities ?? []) {
      // A plugin's skills are not rows here. Each of them has no switch of its own —
      // `skillOverrides` does not reach inside a plugin — so a row would be a tick
      // nobody can move, and the plugin's own row, with its count and its price, is
      // what they ride on. Listing them made 45 dead boxes over 6 real ones.
      if (e.kind === 'skill' && e.plugin) continue
      // Hooks are not rows. The scanner still finds them, and their cost still
      // counts towards the session's total — but the pane shows servers, plugins,
      // skills and rules, and a hook nobody can switch off would be a row that
      // only reports. The profile's single "disable all hooks" switch is where
      // hooks are decided, in the profile pane.
      if (e.kind === 'hook') continue
      const arr = m.get(e.kind) ?? []
      arr.push(e)
      m.set(e.kind, arr)
    }
    for (const arr of m.values()) {
      arr.sort((a, b) => (b.cost.tokens ?? -1) - (a.cost.tokens ?? -1))
    }
    return m
  }, [inventory])

  /** What the pane is actually showing — the head's figure counts rows, not findings. */
  const shown = useMemo(
    () => [...byKind.values()].reduce((n, arr) => n + arr.length, 0),
    [byKind],
  )

  /**
   * Groups the user folded away. Every group starts folded: the pane opens as a
   * list of five headings, not as ninety rows nobody asked to read. A remembered
   * choice is respected — under its own key, so the all-open state saved by the
   * earlier default is not what a first visit reads.
   */
  const [collapsed, setCollapsed] = useState<Set<string>>(() => {
    try {
      const v: unknown = JSON.parse(localStorage.getItem('cc-collapsed-2') ?? '')
      if (Array.isArray(v)) return new Set(v as string[])
    } catch {
      /* nothing saved yet */
    }
    return new Set<string>(['mcp', 'plugin', 'skill', 'memory', PERM_KEY])
  })

  useEffect(() => {
    localStorage.setItem('cc-collapsed-2', JSON.stringify([...collapsed]))
  }, [collapsed])

  const toggleGroup = (key: string) => {
    const next = new Set(collapsed)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    setCollapsed(next)
  }

  // The permission rows carry their own heading name, so they group by that
  // rather than by `kind`, and read under the same fold switch as everything else.
  const permByGroup = useMemo(() => {
    const m = new Map<string, PermissionRow[]>()
    for (const r of inventory?.permissions ?? []) {
      const arr = m.get(r.group) ?? []
      arr.push(r)
      m.set(r.group, arr)
    }
    return m
  }, [inventory])

  /** Description — the second line of a list row. No text — no row. */
  const Desc = ({ text }: { text?: string }) => {
    if (!text) return null
    // The row clamps this to two lines, so the full text stays reachable on hover.
    return (
      <div className="desc" title={text}>
        {text}
      </div>
    )
  }

  if (!inventory) {
    return (
      <section className="pane">
        <div className="pane-head">{t('inv.title')}</div>
        <div className="empty">{t('inv.noProject')}</div>
      </section>
    )
  }

  // The profile stores "all" as a mode, not as a list, so the rows need the
  // inventory's own names to know what "all" currently covers.
  const allMcp = [
    ...new Set((byKind.get('mcp') as Array<Entity & { kind: 'mcp' }> | undefined)?.map((e) => e.name) ?? []),
  ]
  // What the files refuse to load. The toggle builds its list from the boxes as
  // they are drawn, and those names are unticked by the file, not the profile.
  const deniedMcp = [
    ...new Set(
      (byKind.get('mcp') as Array<Entity & { kind: 'mcp' }> | undefined)
        ?.filter((e) => e.offByFile)
        .map((e) => e.name) ?? [],
    ),
  ]
  // Shared with the ⌘K palette — see profile-ops.ts.
  const toggleMcp = (name: string) => onChange(ops.toggleMcp(profile, name, allMcp, deniedMcp))
  // The disk state comes along: a box ticked back to what the disk already says
  // must not become an override — see togglePlugin.
  const togglePlugin = (id: string, enabled: boolean, onDisk: boolean) =>
    onChange(ops.togglePlugin(profile, id, enabled, onDisk))
  const toggleSkill = (name: string, offByFile?: boolean) =>
    onChange(ops.toggleSkill(profile, name, offByFile))
  /**
   * A row that opens a card, reachable without a mouse.
   *
   * `tabIndex` and the key handler, but no `role="button"`: the row also holds a
   * checkbox and a button of its own, and the role would claim the whole strip is
   * one control. Enter and Space open the card, as they would on a button.
   */
  const openRow = (open: () => void) => ({
    tabIndex: 0,
    onClick: open,
    onKeyDown: (ev: KeyboardEvent) => {
      if (ev.key === 'Enter' || ev.key === ' ') {
        ev.preventDefault()
        open()
      }
    },
  })

  /** One checkbox, wherever it sits: the toggle stops the click from reaching the row. */
  const box = (on: boolean, set: (v: boolean) => void, off = false, why?: string) => (
    <input
      type="checkbox"
      checked={on}
      disabled={off}
      {...(off && why ? { title: why } : {})}
      onClick={(ev) => ev.stopPropagation()}
      onChange={(ev) => !off && set(ev.target.checked)}
    />
  )

  return (
    <section className="pane">
      <div className="pane-head">
        <span>{t('inv.title')}</span>
        <span style={{ flex: 1 }} />
        <span className="muted" style={{ textTransform: 'none', letterSpacing: 0 }}>
          {t('inv.elements', { n: shown })}
        </span>
      </div>

      <div className="pane-body">
        {ORDER.map((kind) => {
          const items = byKind.get(kind)
          if (!items?.length) return null
          const shut = collapsed.has(kind)
          return (
            <div key={kind}>
              <button
                className="group-title"
                aria-expanded={!shut}
                title={t('inv.toggleGroup', { name: t(TITLES[kind]) })}
                onClick={() => toggleGroup(kind)}
              >
                <span>
                  <span className="caret">{shut ? '▸' : '▾'}</span>
                  {t(TITLES[kind])}
                </span>
                <span>{items.length}</span>
              </button>

              {shut ? null : (
                <>
                  {kind === 'mcp' &&
                    (items as Array<Entity & { kind: 'mcp' }>).map((e) => {
                      const on = ops.isEntityLive(profile, e)
                      return (
                      <div
                        key={e.name}
                        className={`row click${on ? '' : ' off'}`}
                        {...openRow(() => onOpen('mcp', e.name))}
                      >
                        {box(on, () => toggleMcp(e.name))}
                        <div className="colname">
                          <span className="name">{e.name}</span>
                          <Desc text={e.description} />
                          {/* An unticked box says the server will not start and
                              nothing else. For a project that is half the answer:
                              the switch lives in a file the user never opens, and
                              there is no way to guess which one from the row. */}
                          {e.offBy?.length ? (
                            <div className="desc off" title={t('inv.mcpOffByTitle')}>
                              {t('inv.mcpOffBy', {
                                where: e.offBy.map((r) => `${r.file} · ${r.key}`).join(', '),
                              })}
                            </div>
                          ) : null}
                        </div>
                        <span className="src">{e.scope}</span>
                        <Chip cost={e.cost} />
                        <button
                          className="btn icon"
                          title={t('inv.probeTitle')}
                          disabled={busy}
                          onClick={(ev) => {
                            ev.stopPropagation()
                            onProbe(e.name)
                          }}
                        >
                          <IconBolt />
                        </button>
                      </div>
                      )
                    })}

                  {kind === 'plugin' &&
                    (items as PluginEntity[]).map((e) => {
                      const on = ops.isEntityLive(profile, e)
                      return (
                      <div
                        key={e.id}
                        className={`row click${on ? '' : ' off'}`}
                        {...openRow(() => onOpen('plugin', e.id))}
                      >
                        {box(on, (v) => togglePlugin(e.id, v, e.enabled))}
                        <div className="colname">
                          <span className="name">{e.id}</span>
                          <Desc text={e.description} />
                        </div>
                        <span className="src">{t('inv.skillsAbbrev', { n: e.skillCount })}</span>
                        <Chip cost={e.cost} />
                      </div>
                      )
                    })}

                  {kind === 'skill' &&
                    (items as Array<Entity & { kind: 'skill' }>).map((e) => {
                      const off = !ops.isEntityLive(profile, e)
                      return (
                        <div
                          key={`${e.origin}:${e.name}`}
                          className={`row click${off ? ' off' : ''}`}
                          {...openRow(() => onOpen('skill', e.name, e.source.file))}
                        >
                          {box(!off, () => toggleSkill(e.name, e.offByFile))}
                          <div className="colname">
                            <span className="name">{e.name}</span>
                            <Desc text={e.description} />
                          </div>
                          <span className="chip origin">{e.origin}</span>
                          <Chip cost={e.cost} />
                        </div>
                      )
                    })}

                  {kind === 'memory' &&
                    (items as Array<Entity & { kind: 'memory' }>).map((e) => (
                      <div
                        key={e.path}
                        className={`row click${ops.isEntityLive(profile, e) ? '' : ' off'}`}
                        {...openRow(() => onOpen('memory', e.path, e.path))}
                      >
                        <span className="name">{e.path}</span>
                        {!ops.isEntityLive(profile, e) && <span className="chip">{t('ui.chipOff')}</span>}
                        <Chip cost={e.cost} />
                      </div>
                    ))}
                </>
              )}
            </div>
          )
        })}

        {/* Permissions are not entities: they cost no context, nothing loads
            them, and one row may write two rules. So they are their own block
            rather than a sixth `kind` — same heading, same fold, their own
            three-way control. */}
        {inventory.permissions.length > 0 && (
          <div>
            <button
              className="group-title"
              aria-expanded={!collapsed.has(PERM_KEY)}
              title={t('inv.toggleGroup', { name: t('inv.kindPermissions') })}
              onClick={() => toggleGroup(PERM_KEY)}
            >
              <span>
                <span className="caret">{collapsed.has(PERM_KEY) ? '▸' : '▾'}</span>
                {t('inv.kindPermissions')}
              </span>
              <span>{inventory.permissions.length}</span>
            </button>

            {collapsed.has(PERM_KEY)
              ? null
              : PERM_ORDER.map((group) => {
                  const items = permByGroup.get(group)
                  if (!items?.length) return null
                  return (
                    <div key={group}>
                      <div className="sub-title">{t(PERM_TITLES[group] ?? group)}</div>
                      {items.map((r) => {
                        // What the files say, said in words: it is the answer to
                        // "как настроено сейчас", and it is not what the control
                        // below shows — that one follows the draft.
                        const onDisk = r.state ?? 'allow'
                        const now = t(RULE_LABELS[onDisk])
                        const want = shownState(r, profile.permissions.rules)
                        return (
                        <div className="row perm" key={`${r.name}:${r.rules.join('|')}`}>
                          <div className="colname">
                            <span
                              className="name"
                              title={[r.why, ...r.rules].filter(Boolean).join('\n')}
                            >
                              {r.name}
                            </span>
                            {/* The command is the second line, where every other
                                row puts its description: the name is what the
                                reader is choosing between, this is what it is. */}
                            {r.cmd && (
                              <div className="desc mono" title={r.why || r.cmd}>
                                {r.cmd}
                              </div>
                            )}
                          </div>
                          {r.scope && (
                            <span
                              className="src"
                              // Only the project file is ours to write, so a rule set
                              // at another level stays where it is and the row says so
                              // instead of pretending the choice took.
                              title={t(
                                r.scope === 'project' ? 'inv.ruleScope' : 'inv.ruleScopeLocked',
                                { scope: r.scope, state: now },
                              )}
                            >
                              {r.scope}
                            </span>
                          )}
                          {/* The control follows the draft, so on its own it would
                              read as the state of the machine. It is not: this says
                              what the files say until the write actually happens. */}
                          {want !== onDisk && (
                            <span className="src changed" title={t('inv.ruleChangedTitle', { state: now })}>
                              {t('inv.ruleChanged')}
                            </span>
                          )}
                          <div className="seg">
                            {RULE_STATES.map((k) => (
                              <button
                                key={k}
                                data-k={k}
                                data-on={want === k}
                                title={t(RULE_TITLES[k], { rules: r.rules.join(', ') })}
                                onClick={() => onRule(r.rules, k)}
                              >
                                {t(RULE_LABELS[k])}
                              </button>
                            ))}
                          </div>
                        </div>
                        )
                      })}
                    </div>
                  )
                })}
          </div>
        )}

        {inventory.notes.map((n, i) => (
          <div className="note" key={i}>
            {n}
          </div>
        ))}

        {inventory.unreadable.map((u) => (
          <div className="note" key={u.file}>
            {u.file}: {u.error}
          </div>
        ))}

      </div>
    </section>
  )
}
