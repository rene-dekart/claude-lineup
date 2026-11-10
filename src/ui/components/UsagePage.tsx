import { useEffect, useMemo, useState } from 'react'
import type { SessionStat, Stats } from '../../shared/types.js'
import * as api from '../api.js'
import { useLang, useT } from '../i18n.js'
import { Panel, Pin, barDate, barYears, fill, localDate, moment, num, pick, share } from './StatsPage.js'

/**
 * What was called, how often, and when.
 *
 * A tab of its own, next to the statistics: that page answers "what it cost", this
 * one answers "what was actually used" — the same transcripts read the other way, so
 * the two never have to agree on anything but the sessions they are drawn from.
 *
 * Everything here comes off the transcript names, and nothing from the inventory. An
 * uninstalled plugin still made the calls it made, and joining against today's disk
 * would quietly drop them. A plugin has no line of its own anywhere: what it did is
 * what its own skills and its own MCP servers did, both of which carry the plugin in
 * their name.
 */

/** The spans the chart offers, in days — the same two the statistics page offers. */
const WINDOWS = [7, 30]

type Kind = 'mcp' | 'skill' | 'plugin'

/** One name and what it did, folded over every session in the window. */
interface Row {
  kind: Kind
  name: string
  calls: number
  sessions: number
  /** Epoch ms of the last turn of the newest session that called it. Zero: unknown. */
  last: number
}

/**
 * The plugin behind a name, or nothing when the name belongs to no plugin.
 *
 * Claude Code's own conventions, which is why the name is enough: a plugin's MCP
 * server is `plugin_<plugin>_<server>` (the plugin is the part up to the FIRST
 * underscore — the non-greedy match, the same one `describe` searches by), and a
 * plugin's skill is invoked as `<plugin>:<skill>`. A bare `github` or a bare
 * `firecrawl` name has no colon and no prefix and is nobody's plugin.
 */
function pluginOf(kind: Kind, name: string): string | undefined {
  if (kind === 'skill') return name.includes(':') ? name.slice(0, name.indexOf(':')) : undefined
  if (kind === 'mcp') return /^plugin_(.+?)_/.exec(name)?.[1]
  return name
}

/** How many times one session called one name. */
function callsOf(s: SessionStat, kind: Kind, name: string): number {
  if (kind === 'mcp') return s.usage.mcp[name] ?? 0
  if (kind === 'skill') return s.usage.skill[name] ?? 0
  let n = 0
  for (const [k, v] of Object.entries(s.usage.mcp)) if (pluginOf('mcp', k) === name) n += v
  for (const [k, v] of Object.entries(s.usage.skill)) if (pluginOf('skill', k) === name) n += v
  return n
}

/** The session's own total, with nothing picked: every call it made, each counted once. */
function totalCalls(s: SessionStat): number {
  let n = 0
  for (const v of Object.values(s.usage.mcp)) n += v
  for (const v of Object.values(s.usage.skill)) n += v
  return n
}

/** Every name of one kind in the list, by calls. No cap: the list is short, and a
 *  name you cannot see is a name you cannot click. */
function rowsOf(all: SessionStat[], kind: Kind): Row[] {
  const out = new Map<string, Row>()
  const bump = (name: string, s: SessionStat, n: number): void => {
    const r = out.get(name) ?? { kind, name, calls: 0, sessions: 0, last: 0 }
    r.calls += n
    r.sessions++
    r.last = Math.max(r.last, s.end)
    out.set(name, r)
  }
  for (const s of all) {
    for (const [name, n] of Object.entries(s.usage.mcp)) {
      if (kind === 'mcp') bump(name, s, n)
      else if (kind === 'plugin') {
        const p = pluginOf('mcp', name)
        if (p) bump(p, s, n)
      }
    }
    for (const [name, n] of Object.entries(s.usage.skill)) {
      if (kind === 'skill') bump(name, s, n)
      else if (kind === 'plugin') {
        const p = pluginOf('skill', name)
        if (p) bump(p, s, n)
      }
    }
  }
  return [...out.values()].sort((a, b) => b.calls - a.calls)
}

/** What is picked: one name of one kind. Absent: everything. */
interface Sel {
  kind: Kind
  name: string
}

interface DayCalls {
  date: string
  calls: number
  sessions: number
}

/**
 * The chart's window: `days` local days ending today, oldest first and zero-filled,
 * so a day with nothing in it is an empty slot and not a gap in the row. `setDate`,
 * not millisecond arithmetic: subtracting 24 h across a DST change lands on the same
 * calendar date twice and leaves a hole.
 */
function callsWindow(now: number, days: number): DayCalls[] {
  const midnight = new Date(now)
  midnight.setHours(0, 0, 0, 0)
  const out: DayCalls[] = []
  for (let back = days - 1; back >= 0; back--) {
    const d = new Date(midnight)
    d.setDate(midnight.getDate() - back)
    out.push({ date: localDate(d.getTime()), calls: 0, sessions: 0 })
  }
  return out
}

interface View {
  /** Sessions that used the picked name, or all of them when nothing is picked. */
  sessions: SessionStat[]
  calls: number
  /** Distinct names these sessions used: servers, skills, plugins. */
  mcp: number
  skills: number
  plugins: number
  first: number
  /** The day chart, narrowed to the same sessions as the figures above it. */
  byDay: DayCalls[]
}

/**
 * The same fold the statistics page does, over the same flat list: what a name did,
 * and — with a name picked — the figures of the sessions that called it.
 *
 * Every figure above the tables comes from here, the four tiles included, so a pick
 * narrows all of them together and none of them can disagree with the call count
 * beside it. With a name picked the three counters of names answer about that name
 * alone — one server and no skill for a server, one skill for a skill, and the plugin
 * that owns it — because a call of anything else is not a call of this. The tables are
 * not drawn from here: they are always the whole list, so a pick marks a line and never
 * removes one.
 */
function fold(all: SessionStat[], sel: Sel | null, days: number, now: number): View {
  const callsIn = (s: SessionStat): number => (sel ? callsOf(s, sel.kind, sel.name) : totalCalls(s))
  const sessions = sel ? all.filter((s) => callsIn(s) > 0) : all
  /** Whether a name belongs to the view: the picked one, or every name when nothing is
   *  picked. A picked name of another kind is not part of it. */
  const mine = (kind: Kind, name: string): boolean =>
    sel === null || (sel.kind === kind && sel.name === name)

  const byDay = callsWindow(now, days)
  const index = new Map(byDay.map((d) => [d.date, d]))
  const mcp = new Set<string>()
  const skills = new Set<string>()
  const plugins = new Set<string>()
  let calls = 0
  let first = 0

  for (const s of sessions) {
    const n = callsIn(s)
    calls += n
    if (s.start && (first === 0 || s.start < first)) first = s.start
    for (const [name, k] of Object.entries(s.usage.mcp)) {
      if (k === 0 || !mine('mcp', name)) continue
      mcp.add(name)
      const p = pluginOf('mcp', name)
      if (p) plugins.add(p)
    }
    for (const [name, k] of Object.entries(s.usage.skill)) {
      if (k === 0 || !mine('skill', name)) continue
      skills.add(name)
      const p = pluginOf('skill', name)
      if (p) plugins.add(p)
    }

    if (!s.start) continue
    // One session lands on one bar: the day it started, the same rule the statistics
    // page uses. Spreading a three-day session across three bars would put a figure
    // on each that nothing measured.
    const day = index.get(localDate(s.start))
    if (!day) continue
    day.calls += n
    day.sessions++
  }

  // The picked name's own owner. Nothing in the loops above matches a picked plugin —
  // it is not a server and not a skill, it is what the two have in common — and for the
  // names that do match it is the plugin they already contributed, so this adds nothing.
  if (sel) {
    const p = pluginOf(sel.kind, sel.name)
    if (p) plugins.add(p)
  }

  return {
    sessions,
    calls,
    mcp: mcp.size,
    skills: skills.size,
    plugins: plugins.size,
    first,
    byDay,
  }
}

/** The three tables, and the one fold each of them draws. `rows` is the whole list:
 *  picking a name narrows the figures and leaves every table line on screen. */
const TABLES: Array<{ kind: Kind; title: string }> = [
  { kind: 'mcp', title: 'usage.byMcp' },
  { kind: 'skill', title: 'usage.bySkill' },
  { kind: 'plugin', title: 'usage.byPlugin' },
]

export function UsagePage() {
  const t = useT()
  const lang = useLang()
  const [stats, setStats] = useState<Stats | null>(null)
  const [error, setError] = useState('')
  const [sel, setSel] = useState<Sel | null>(null)
  const [days, setDays] = useState(30)

  useEffect(() => {
    api
      .getStats()
      .then(setStats)
      .catch((e: unknown) => setError((e as Error).message))
    // The transcripts are already read for the statistics page and the server serves
    // both from one scan, so opening this tab costs a request and no disk.
  }, [])

  const sessions = stats?.sessions ?? []
  // Two folds, as on the statistics page: the tables come from the whole list, the
  // figures from the part of it that matches. Picking a name moves nothing.
  const view = useMemo(() => fold(sessions, null, days, Date.now()), [sessions, days])
  const shown = useMemo(
    () => (sel ? fold(sessions, sel, days, Date.now()) : view),
    [sessions, sel, days, view],
  )

  /** A name that is already picked is unpicked by clicking it again. */
  const mark = (kind: Kind, name: string) =>
    setSel((s) => (s?.kind === kind && s.name === name ? null : { kind, name }))

  // Each table's own total: what its rows hold a share of. A plugin's calls are the
  // calls of its own servers and skills, so the three tables do not add up to the same
  // number — and a share measured against another table's total would be a percentage
  // of something this line is not part of.
  const tables = TABLES.map((tb) => {
    const rows = rowsOf(sessions, tb.kind)
    return { ...tb, rows, total: rows.reduce((n, r) => n + r.calls, 0) }
  })
  const tiles: Array<[string, string]> = [
    [t('usage.calls'), num(lang, shown.calls)],
    [t('usage.mcp'), num(lang, shown.mcp)],
    [t('usage.skills'), num(lang, shown.skills)],
    [t('usage.plugins'), num(lang, shown.plugins)],
  ]

  const maxDay = Math.max(0, ...shown.byDay.map((d) => d.calls))
  const empty = stats !== null && sessions.length === 0

  return (
    <div className="page">
      <div className="page-head">
        <span className="page-title">{t('usage.title')}</span>
        <div className="grow" />
        <div className="seg">
          {WINDOWS.map((n) => (
            <button key={n} data-on={days === n} title={t('usage.window', { n })} onClick={() => setDays(n)}>
              {n}
            </button>
          ))}
        </div>
      </div>

      <div className="page-body">
        {!stats && !error && <div className="empty">{t('usage.loading')}</div>}
        {error && <div className="empty">{t('usage.failed', { message: error })}</div>}
        {/* Nothing read at all is not a page of zeroes — it is one line. */}
        {empty && <div className="empty">{t('usage.empty')}</div>}
        {stats && !empty ? (
          <>
            <Pin>
              <div className="stats-tiles">
                {tiles.map(([label, value]) => (
                  <div className="stats-tile" key={label}>
                    <span className="stats-num">{value}</span>
                    <span className="stats-cap">{label}</span>
                  </div>
                ))}
              </div>

              <div className="stats-meta">
                {[
                  shown.first ? t('usage.first', { date: moment(lang, shown.first) }) : '',
                  // How much of the history the picked name accounts for. With nothing
                  // picked the two figures are the same and the line would read as a
                  // tautology, which is why it is only here when something is.
                  sel ? t('usage.inSessions', { n: num(lang, shown.sessions.length) }) : '',
                ]
                  .filter(Boolean)
                  .join(' · ')}
              </div>

              <Panel title={t('usage.byDay')} wide>
                {/* The top of the scale, where an axis puts it. */}
                <div className="stats-peak">{num(lang, maxDay)}</div>
                {/* Bars are calls, one slot per day, zero-filled, so the shape of the
                    week is a shape and not a gap. */}
                <div className="stats-bars">
                  {shown.byDay.map((d) => {
                    const tip = t('usage.barTip', {
                      date: d.date,
                      calls: num(lang, d.calls),
                      sessions: num(lang, d.sessions),
                    })
                    return (
                      <button className="stats-slot" key={d.date} title={tip} aria-label={tip}>
                        {/* `max()`, not a bare percentage: a day with a handful of calls
                            against a busy peak rounds to nothing and would read as an
                            empty day. */}
                        <span
                          className="stats-bar"
                          style={{ height: d.calls > 0 ? `max(2px, ${(d.calls / maxDay) * 100}%)` : '0' }}
                        />
                      </button>
                    )
                  })}
                </div>
                <div className="stats-days">
                  {shown.byDay.map((d) => (
                    <span key={d.date}>{barDate(lang, d.date)}</span>
                  ))}
                </div>
                <div className="stats-axis">
                  <span>{t('usage.window', { n: days })}</span>
                  <span className="tiny">{barYears(shown.byDay)}</span>
                </div>
              </Panel>
            </Pin>

            {/* The first table takes the whole width and the two that follow stand
                side by side, ending the page. Three tables of different lengths in two
                columns would leave the column that runs out first hanging above the
                next block, with a hole under it. */}
            <div className="page-grid">
              {tables.map((tb) => (
                <Panel key={tb.kind} title={t(tb.title)} wide={tb.kind === 'mcp'}>
                  <div className="row stats-head">
                    <div className="colname">{t('usage.name')}</div>
                    <span className="src usage-num">{t('usage.calls')}</span>
                    <span className="src stats-share">%</span>
                    <span className="src usage-num">{t('usage.colSessions')}</span>
                    <span className="src usage-date">{t('usage.colLast')}</span>
                  </div>
                  {tb.rows.map((r) => (
                    <div
                      className={`row fill click${sel?.kind === r.kind && sel.name === r.name ? ' sel' : ''}`}
                      key={r.name}
                      style={fill(r.calls, tb.rows[0]?.calls ?? 0)}
                      {...pick(() => mark(r.kind, r.name))}
                    >
                      <div className="colname">
                        <span className="name" title={r.name}>
                          {r.name}
                        </span>
                      </div>
                      <span className="src usage-num">{num(lang, r.calls)}</span>
                      <span className="src stats-share">{share(lang, r.calls, tb.total)}</span>
                      <span className="src usage-num">{num(lang, r.sessions)}</span>
                      {/* A name that was never called has no date, and 1970 would be a
                          figure in place of the fact that there is none. */}
                      <span className="src usage-date">
                        {r.last ? moment(lang, r.last) : t('usage.never')}
                      </span>
                    </div>
                  ))}
                </Panel>
              ))}
            </div>
          </>
        ) : null}
      </div>
    </div>
  )
}
