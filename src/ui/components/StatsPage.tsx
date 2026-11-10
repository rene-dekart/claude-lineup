import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import type { Lang } from '../../shared/i18n/index.js'
import type {
  DayStat,
  HistorySize,
  ProjectStat,
  SessionStat,
  Stats,
  TokenCounts,
} from '../../shared/types.js'
import * as api from '../api.js'
import { useLang, useT } from '../i18n.js'

/** Past this a count goes short: `1 234` is worth its digits, `340K` is not. */
const COMPACT_FROM = 10_000

/** The spans the day chart offers, in days. */
const WINDOWS = [7, 30]

/** How many projects the table lists before it starts counting the rest. */
const TOP_PROJECTS = 12

const NUM = new Map<string, Intl.NumberFormat>()

/**
 * A count as the panel shows it: short above the threshold, in the units of the
 * current language (`340K`, `340 тыс.`), grouped below it.
 */
export function num(lang: Lang, n: number): string {
  const big = n >= COMPACT_FROM
  const key = `${lang}:${big}`
  let f = NUM.get(key)
  if (!f) {
    f = big
      ? new Intl.NumberFormat(lang, { notation: 'compact', maximumFractionDigits: 1 })
      : new Intl.NumberFormat(lang)
    NUM.set(key, f)
  }
  return f.format(n)
}

const PERC = new Map<string, Intl.NumberFormat>()

/**
 * A part of a whole, as a percentage — the share column, next to the figure it
 * describes.
 *
 * Rounded to the unit except below a tenth, where the decimal is the whole point:
 * `0 %` and `0,4 %` are two different facts about a name, and a table of forty MCP
 * servers is mostly the second kind.
 */
export function share(lang: Lang, part: number, whole: number): string {
  if (whole <= 0) return ''
  const fine = part / whole < 0.1
  const key = `${lang}:${fine}`
  let f = PERC.get(key)
  if (!f) {
    f = new Intl.NumberFormat(lang, {
      style: 'percent',
      maximumFractionDigits: fine ? 1 : 0,
    })
    PERC.set(key, f)
  }
  return f.format(part / whole)
}

/**
 * How far the fill behind a row reaches: the row's own figure against the largest in
 * its table, so the top line is full and every other is read against it.
 *
 * A custom property and not a width: the fill is the row's own background, so nothing
 * on the line has to move out of the way for it, and the figures keep their columns.
 */
export function fill(value: number, max: number): CSSProperties {
  return { '--fill': `${max > 0 ? (value / max) * 100 : 0}%` } as CSSProperties
}

const DATES = new Map<Lang, Intl.DateTimeFormat>()
/** `since` is epoch ms — a moment, so it goes through Intl and not out as ISO. */
export function moment(lang: Lang, ms: number): string {
  let f = DATES.get(lang)
  if (!f) {
    f = new Intl.DateTimeFormat(lang, { dateStyle: 'medium' })
    DATES.set(lang, f)
  }
  return f.format(ms)
}

const UNITS: Record<Lang, string[]> = {
  ru: ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'],
  en: ['B', 'KB', 'MB', 'GB', 'TB'],
}

/**
 * A pile of bytes at a human scale. Written out by hand because Intl's own byte
 * unit does not scale: `notation: 'compact'` on 1.1 GB prints "1,1 млрд Б", which
 * is a number nobody reads as a size.
 */
function bytes(lang: Lang, n: number): string {
  let i = 0
  let v = n
  while (v >= 1024 && i < UNITS[lang].length - 1) {
    v /= 1024
    i++
  }
  const f = new Intl.NumberFormat(lang, { maximumFractionDigits: i === 0 ? 0 : 1 })
  return `${f.format(v)} ${UNITS[lang][i]}`
}

// ── the fold ────────────────────────────────────────────────────────────────
//
// The server hands over one flat record per session and nothing else. Every figure
// on screen — the tiles, the bars, the tables — is a fold over that same list, and
// a narrowed view is the same arithmetic over a shorter array, so a figure can
// never disagree with the total it came from.

function emptyTokens(): TokenCounts {
  return { input: 0, output: 0, cacheRead: 0, cacheCreate: 0, thinking: 0 }
}

function addTokens(into: TokenCounts, from: TokenCounts): void {
  into.input += from.input
  into.output += from.output
  into.cacheRead += from.cacheRead
  into.cacheCreate += from.cacheCreate
  // Thinking is already inside output. It gets its own accumulator so it can be
  // shown apart, never a fifth column added to the four.
  into.thinking += from.thinking
}

const pad = (n: number) => String(n).padStart(2, '0')

/** YYYY-MM-DD in LOCAL time: a day is what the clock on the wall said. */
export function localDate(ms: number): string {
  const d = new Date(ms)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

const DAY_MONTH = new Map<Lang, Intl.DateTimeFormat>()

function fmt(
  cache: Map<Lang, Intl.DateTimeFormat>,
  lang: Lang,
  opts: Intl.DateTimeFormatOptions,
): Intl.DateTimeFormat {
  let f = cache.get(lang)
  if (!f) {
    f = new Intl.DateTimeFormat(lang, opts)
    cache.set(lang, f)
  }
  return f
}

/**
 * The date under one bar, on one line: the day and the month, in the short numeric
 * form of the current language (`09.10`, `10/09`).
 *
 * Numeric and not a month name because the slot is only about 39 px wide: `10 сент.`
 * needs 41 and wraps, taking two lines where `9 сент.` takes one, so the row of
 * labels comes out ragged — one label sitting a line higher than its neighbours.
 * Four or five characters are the same width in every language and in every month,
 * so the row stays even. The year is not on the bar: it is the same for the whole
 * window and belongs to the axis, which says it once.
 */
export function barDate(lang: Lang, date: string): string {
  // Noon, not midnight: a bare `YYYY-MM-DD` parses as UTC and lands on the previous
  // day anywhere west of Greenwich, so the label would name the wrong one.
  const ms = new Date(`${date}T12:00:00`).getTime()
  return fmt(DAY_MONTH, lang, { day: '2-digit', month: '2-digit' }).format(ms)
}

/**
 * The years the window covers: one, or both when it crosses New Year. Read off the
 * dates themselves — the window is drawn from the clock, not from a calendar.
 */
export function barYears(byDay: Array<{ date: string }>): string {
  const first = byDay[0]?.date.slice(0, 4)
  const last = byDay[byDay.length - 1]?.date.slice(0, 4)
  if (!first || !last) return ''
  return first === last ? first : `${first} — ${last}`
}

/**
 * The window the bars are drawn from: `days` local days ending today, oldest
 * first and zero-filled, so a day with nothing in it is an empty slot and not a
 * gap in the row.
 */
function dayWindow(now: number, days: number): DayStat[] {
  const midnight = new Date(now)
  midnight.setHours(0, 0, 0, 0)
  const out: DayStat[] = []
  for (let back = days - 1; back >= 0; back--) {
    // setDate, not millisecond arithmetic: subtracting 24 h across a DST change
    // lands on the same calendar date twice and leaves a hole in the window.
    const d = new Date(midnight)
    d.setDate(midnight.getDate() - back)
    out.push({ date: localDate(d.getTime()), sessions: 0, messages: 0, input: 0, output: 0 })
  }
  return out
}

/** What is picked out on the panel. It marks the row you clicked and narrows the
 *  figures at the top of the page; it never removes a row, a bar or a table line,
 *  so nothing on screen moves when you click. An absent key is "nothing picked",
 *  and clicking the same row again clears it, the way the inventory's rows do. */
interface Sel {
  project?: string
  model?: string
  day?: string
}

/** Whether anything is picked at all — the difference between the list's own
 *  totals and the totals of the part of it that matches. */
const picked = (f: Sel): boolean =>
  f.project !== undefined || f.model !== undefined || f.day !== undefined

interface View {
  sessions: SessionStat[]
  tokens: TokenCounts
  messages: number
  projects: ProjectStat[]
  byDay: DayStat[]
  byModel: Array<[string, TokenCounts]>
  since: number
  longestMs: number
}

function fold(all: SessionStat[], f: Sel, days: number, now: number): View {
  const sessions = all.filter(
    (s) =>
      (f.project === undefined || s.project === f.project) &&
      (f.model === undefined || s.models[f.model] !== undefined) &&
      (f.day === undefined || localDate(s.start) === f.day),
  )

  /**
   * The session's four columns — unless a model is named, in which case they are
   * that model's own. `usage` is reported per API call and each call names the
   * model that answered it, so charging the whole session to one model of it
   * would put the other models' tokens in a figure that claims to be about this one.
   */
  const tokensOf = (s: SessionStat): TokenCounts =>
    f.model === undefined ? s.tokens : (s.models[f.model] ?? emptyTokens())

  const tokens = emptyTokens()
  const byModel = new Map<string, TokenCounts>()
  const projects = new Map<string, ProjectStat>()
  let messages = 0
  let since = 0
  let longestMs = 0

  for (const s of sessions) {
    messages += s.messages
    longestMs = Math.max(longestMs, s.longestMs)
    if (s.start && (since === 0 || s.start < since)) since = s.start
    addTokens(tokens, tokensOf(s))

    // Per model, not per session: `usage` is reported per API call and each call
    // names the model that answered it, so the split below is measured, not charged.
    for (const [model, tk] of Object.entries(s.models)) {
      const acc = byModel.get(model)
      if (acc) addTokens(acc, tk)
      else byModel.set(model, { ...tk })
    }

    // A session with no cwd anywhere in its lines is still real money. It gets one
    // honest label of its own rather than being dropped from a list that adds up
    // to a total the dropped session is part of.
    const key = s.project || ''
    const p =
      projects.get(key) ??
      { path: key, sessions: 0, messages: 0, tokens: emptyTokens(), last: 0 }
    p.sessions++
    p.messages += s.messages
    addTokens(p.tokens, tokensOf(s))
    p.last = Math.max(p.last, s.end)
    projects.set(key, p)
  }

  const byDay = dayWindow(now, days)
  const index = new Map(byDay.map((d) => [d.date, d]))
  for (const s of sessions) {
    if (!s.start) continue
    // One session lands on one bar: the day it started. Spreading a three-day
    // session across three bars would put a figure on each that nothing measured.
    const day = index.get(localDate(s.start))
    if (!day) continue
    const tk = tokensOf(s)
    day.sessions++
    day.messages += s.messages
    // The two full-rate columns only. Cache traffic is a fraction of an input
    // token; adding it here would put a figure on the bar that no day ever cost.
    day.input += tk.input
    day.output += tk.output
  }

  return {
    sessions,
    tokens,
    messages,
    byDay,
    since,
    longestMs,
    projects: [...projects.values()].sort((a, b) => b.tokens.output - a.tokens.output),
    byModel: [...byModel.entries()].sort((a, b) => b[1].output - a[1].output),
  }
}

// ── the page ─────────────────────────────────────────────────────────────────

/**
 * What the sessions cost, cut any way you like.
 *
 * A page of its own, not a window over the workspace: reading it is a different
 * job from configuring a project, and the three panes behind it are noise while
 * you do it. It takes the whole area under the topbar and scrolls on its own, and
 * it has no close button — the rail is the way back, as it is from the other three.
 *
 * One fact table, folded on the client. Clicking a project, a model or a bar marks
 * that row and folds the figures to it — the tiles, the line under them and the day
 * chart all answer for the picked part of the list. Both tables keep every line they
 * had, because a list that loses its rows is a list you cannot click your way back
 * out of. The four token columns are never added into one number anywhere in here —
 * a cache read is a fraction of an input token, so a single total would overstate
 * the bill by an order of magnitude. They stay four tiles.
 *
 * The first read walks every transcript on the machine, so the page asks for
 * them when it opens, not at app load.
 */
export function StatsPage() {
  const t = useT()
  const [stats, setStats] = useState<Stats | null>(null)
  const [error, setError] = useState('')
  const [sel, setSel] = useState<Sel>({})
  const [days, setDays] = useState(30)

  async function load() {
    try {
      setStats(await api.getStats())
    } catch (e) {
      setError((e as Error).message)
    }
  }

  useEffect(() => {
    void load()
    // Reading is the only thing this page does on open, and `load` would re-run
    // on a new identity each render.
  }, [])

  // `Date.now()` inside the fold is what places the window; it is read on the same
  // render the data arrives on, and nothing re-renders on a clock.
  //
  // Two folds, not one: the two tables are drawn from the whole list, the figures —
  // the tiles, the line beneath them and the day chart — from the picked part of it.
  // Picking something changes figures and a color, and moves nothing: the chart keeps
  // its thirty slots and its height, and the tables keep every row.
  const view = useMemo(
    () => (stats ? fold(stats.sessions, {}, days, Date.now()) : null),
    [stats, days],
  )
  const totals = useMemo(
    () => (stats && picked(sel) ? fold(stats.sessions, sel, days, Date.now()) : view),
    [stats, sel, days, view],
  )

  /** A row that is already marked is unmarked by clicking it again. */
  const mark = (key: keyof Sel, value: string) =>
    setSel((s) => ({ ...s, [key]: s[key] === value ? undefined : value }))

  const empty = stats !== null && stats.sessions.length === 0

  return (
    <div className="page">
      <div className="page-head">
        <span className="page-title">{t('stats.title')}</span>
        <div className="grow" />
        <div className="seg">
          {WINDOWS.map((n) => (
            <button
              key={n}
              data-on={days === n}
              title={t('stats.window', { n })}
              onClick={() => setDays(n)}
            >
              {n}
            </button>
          ))}
        </div>
      </div>

      <div className="page-body">
        {!stats && !error && <div className="empty">{t('stats.loading')}</div>}
        {error && <div className="empty">{t('stats.failed', { message: error })}</div>}
        {/* Nothing read at all is not a dashboard of zeroes — it is one line. */}
        {empty && <div className="empty">{t('stats.empty')}</div>}
        {view && totals && !empty ? (
          <Dashboard
            view={view}
            totals={totals}
            sel={sel}
            days={days}
            savings={stats?.savings ?? []}
            history={stats?.history ?? null}
            notes={stats?.notes ?? []}
            onMark={mark}
            onReload={load}
          />
        ) : null}
      </div>
    </div>
  )
}

// ── the dashboard ────────────────────────────────────────────────────────────

interface DashProps {
  /** The whole list: what both tables are drawn from. */
  view: View
  /** The picked part of it — the same object as `view` when nothing is picked.
   *  What the tiles, the line under them and the day chart are drawn from. */
  totals: View
  sel: Sel
  days: number
  savings: Stats['savings']
  history: HistorySize | null
  notes: string[]
  onMark: (key: keyof Sel, value: string) => void
  onReload: () => void
}

function Dashboard({
  view,
  totals,
  sel,
  days,
  savings,
  history,
  notes,
  onMark,
  onReload,
}: DashProps) {
  const t = useT()
  // The formatters need the language itself, not only the lookup — as state, so a
  // switch reformats the figures along with the words around them.
  const lang = useLang()

  const tiles: Array<[string, string]> = [
    [t('stats.sessions'), num(lang, totals.sessions.length)],
    [t('stats.messages'), num(lang, totals.messages)],
    [t('stats.projects'), num(lang, totals.projects.length)],
    [t('stats.input'), num(lang, totals.tokens.input)],
    [t('stats.output'), num(lang, totals.tokens.output)],
    [t('stats.cacheRead'), num(lang, totals.tokens.cacheRead)],
    [t('stats.cacheWrite'), num(lang, totals.tokens.cacheCreate)],
  ]

  const projects = view.projects.slice(0, TOP_PROJECTS)
  const hidden = view.projects.length - projects.length
  // The fills and the shares are read against the whole table, not against the part of
  // it on screen: the projects that were cut are still part of the total they hold a
  // share of, and a share measured against the visible dozen would grow when the list
  // is trimmed.
  const projectOut = view.projects.reduce((n, p) => n + p.tokens.output, 0)
  const modelOut = view.byModel.reduce((n, [, tk]) => n + tk.output, 0)

  const maxDay = Math.max(0, ...totals.byDay.map((d) => d.input + d.output))

  /** The source id is the i18n key suffix. An id we have no entry for is still a
   *  name, so it shows as itself rather than as a missing `stats.tool.…` key. */
  const tool = (id: string): string => {
    const key = `stats.tool.${id}`
    const s = t(key)
    return s === key ? id : s
  }

  return (
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
            totals.since ? t('stats.since', { date: moment(lang, totals.since) }) : '',
            totals.longestMs > 0
              ? t('stats.longest', { min: Math.round(totals.longestMs / 60_000) })
              : '',
          ]
            .filter(Boolean)
            .join(' · ')}
        </div>

        <Panel title={t('stats.byDay')} wide>
          {/* The top of the scale, where an axis puts it: a bar is then read as a figure
              and not only as taller than the one beside it. */}
          <div className="stats-peak">{num(lang, maxDay)}</div>
          {/* The bar is tokens at full rate — input and output, no cache — so it
              reads as the bill it is. `byDay` is a zero-filled window, so there is
              one slot per day and no gaps: a day with nothing in it is an empty slot. */}
          <div className="stats-bars">
            {totals.byDay.map((d) => {
              const rate = d.input + d.output
              const tip = t('stats.barTip', {
                date: d.date,
                sessions: num(lang, d.sessions),
                messages: num(lang, d.messages),
                input: num(lang, d.input),
                output: num(lang, d.output),
              })
              return (
                <button
                  className="stats-slot"
                  key={d.date}
                  data-on={sel.day === d.date}
                  title={tip}
                  aria-label={tip}
                  onClick={() => onMark('day', d.date)}
                >
                  {/* `max()`, not a bare percentage: a day with a few thousand
                      tokens against a peak of 400M rounds to nothing on screen and
                      would read as an empty day. */}
                  <span
                    className="stats-bar"
                    style={{ height: rate > 0 ? `max(2px, ${(rate / maxDay) * 100}%)` : '0' }}
                  />
                </button>
              )
            })}
          </div>
          {/* One label per slot, in a row stretched with the same slack as the bars
              themselves, so each day stands under the bar it names — the empty days
              included, since a nameless empty slot reads as a gap in the row. */}
          <div className="stats-days">
            {totals.byDay.map((d) => (
              <span key={d.date}>{barDate(lang, d.date)}</span>
            ))}
          </div>
          <div className="stats-axis">
            <span>{t('stats.window', { n: days })}</span>
            <span className="tiny">{barYears(totals.byDay)}</span>
          </div>
        </Panel>
      </Pin>

      <div className="page-grid">
        <Panel title={t('stats.byProject')} wide>
          <div className="row stats-head">
            <div className="colname">{t('stats.path')}</div>
            <span className="src">{t('stats.sessions')}</span>
            <span className="src">{t('stats.inOut')}</span>
            <span className="src stats-share">%</span>
          </div>
          {projects.map((p) => (
            <div
              className={`row fill click${sel.project === p.path ? ' sel' : ''}`}
              key={p.path}
              style={fill(p.tokens.output, projects[0]?.tokens.output ?? 0)}
              {...pick(() => onMark('project', p.path))}
            >
              <div className="colname">
                {/* The tail is the part that names the project, so the path is
                    clipped from the left; the whole of it stays in the tooltip. */}
                <span className="name stats-path" title={p.path}>
                  {p.path || t('stats.noProject')}
                </span>
              </div>
              <span className="src">{num(lang, p.sessions)}</span>
              {/* Two figures, not their sum: see the note on the token tiles. */}
              <span className="src">
                {num(lang, p.tokens.input)} / {num(lang, p.tokens.output)}
              </span>
              <span className="src stats-share">{share(lang, p.tokens.output, projectOut)}</span>
            </div>
          ))}
          {hidden > 0 ? <div className="stats-more">{t('stats.notShown', { n: hidden })}</div> : null}
        </Panel>

        <Panel title={t('stats.byModel')}>
          {/* A bare `38,2 млн` beside a model name does not say which of the four
              columns it is, and a model list is short enough that one header row
              costs nothing. */}
          <div className="row stats-head">
            <div className="colname">{t('stats.model')}</div>
            <span className="src">{t('stats.output')}</span>
            <span className="src stats-share">%</span>
          </div>
          {view.byModel.map(([model, tk]) => (
            <div
              className={`row fill click${sel.model === model ? ' sel' : ''}`}
              key={model}
              style={fill(tk.output, view.byModel[0]?.[1].output ?? 0)}
              {...pick(() => onMark('model', model))}
            >
              <div className="colname">
                <span className="name" title={model}>
                  {model}
                </span>
              </div>
              <span className="src">{num(lang, tk.output)}</span>
              <span className="src stats-share">{share(lang, tk.output, modelOut)}</span>
            </div>
          ))}
        </Panel>

        <Panel title={t('stats.savings')}>
          {savings.map((src) => (
            <div className="row" key={src.id}>
              <div className="colname">
                <span className="name">{tool(src.id)}</span>
                {src.note ? <div className="desc">{src.note}</div> : null}
              </div>
              {/* A tool that reports nothing is not a tool that saves nothing: an
                  absent figure says "no data", a bold 0 would claim the opposite. */}
              {src.saved === undefined ? (
                <span className="src muted">{t('stats.noData')}</span>
              ) : (
                <span className="src">{num(lang, src.saved)}</span>
              )}
              {/* One span and not two: `.row` puts no gap between its children, so a
                  pair of them reads as the single run-on phrase "of 67.8M calls: 19.2K". */}
              <span className="tiny">
                {[
                  src.total !== undefined ? t('stats.savedOf', { total: num(lang, src.total) }) : '',
                  src.calls !== undefined ? t('stats.calls', { calls: num(lang, src.calls) }) : '',
                ]
                  .filter(Boolean)
                  .join(' · ')}
              </span>
            </div>
          ))}
        </Panel>

        {/* What could not be read. Never a silent zero, so never hidden. */}
        {notes.length > 0 ? (
          <Panel title={t('stats.notes')} wide>
            {notes.map((n) => (
              <div className="stats-note" key={n}>
                {n}
              </div>
            ))}
          </Panel>
        ) : null}

        {history ? <HistoryPanel history={history} onReload={onReload} /> : null}
      </div>
    </>
  )
}

/**
 * The transcripts themselves: how big the pile is, what the retention rule has
 * not taken yet, and a button that takes it now.
 *
 * This is the answer to "why does my history start a month ago" — Claude Code
 * prunes the folder on its own schedule, so the oldest session on disk is a fact
 * about the retention rule rather than about when the tool was installed. The
 * deletion is asked once, in place, and never a second time.
 */
function HistoryPanel({ history, onReload }: { history: HistorySize; onReload: () => void }) {
  const t = useT()
  const lang = useLang()
  const [armed, setArmed] = useState(false)
  const [result, setResult] = useState('')

  const count = { files: num(lang, history.oldFiles), size: bytes(lang, history.oldBytes) }

  async function run() {
    // Disarmed before the request goes out: a slow unlink must not be clickable twice.
    setArmed(false)
    try {
      const r = await api.cleanHistory(history.retentionDays)
      setResult(
        t('stats.cleaned', { files: num(lang, r.deleted), size: bytes(lang, r.bytes) }) +
          (r.failed > 0 ? ` · ${t('stats.cleanupFailed', { failed: num(lang, r.failed) })}` : ''),
      )
      // The pile on screen is now the old one. Re-read rather than subtract: the
      // scan is where the truth is, and a hand-edited figure would be our arithmetic.
      onReload()
    } catch (e) {
      setResult((e as Error).message)
    }
  }

  return (
    <Panel title={t('stats.history')} wide>
      <div className="row">
        <div className="colname">
          <span className="name">{t('stats.onDisk')}</span>
        </div>
        <span className="src">
          {t('stats.filesSize', {
            files: num(lang, history.files),
            size: bytes(lang, history.bytes),
          })}
        </span>
      </div>

      <div className="row">
        <div className="colname">
          <span className="name">{t('stats.olderThan', { days: history.retentionDays })}</span>
        </div>
        <span className="src">{t('stats.filesSize', count)}</span>
        {/* Nothing to delete is not a button that deletes nothing: the control is
            simply absent, and the line beside it says why there is nothing. */}
        {history.oldFiles > 0 ? (
          <button
            className={armed ? 'btn armed' : 'btn'}
            title={t('stats.cleanupAsk', count)}
            onClick={() => (armed ? void run() : setArmed(true))}
          >
            {t(armed ? 'stats.cleanupArmed' : 'stats.cleanup', count)}
          </button>
        ) : null}
      </div>

      {result ? <div className="stats-note">{result}</div> : null}
    </Panel>
  )
}

/**
 * The figures and the chart, held to the top of the page while the tables below them
 * scroll. Picking a row down in a table is a look at what the figures do, and figures
 * that answer from off the screen answer nothing.
 *
 * Opaque, and not only for looks: the rows pass under it, and without a background of
 * its own they would show through the gaps between the tiles and between the panels.
 * Shared with the usage page.
 *
 * Its height is published as `--pin-h` for the column heads in the tables below, which
 * hold their line just under it. How tall the slab is depends on how the tiles wrap, so
 * no constant would do; the observer is in this component because this is the only
 * place that has the element.
 */
export function Pin({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const root = document.documentElement
    const put = () => root.style.setProperty('--pin-h', `${el.offsetHeight}px`)
    put()
    const ro = new ResizeObserver(put)
    ro.observe(el)
    return () => {
      ro.disconnect()
      root.style.removeProperty('--pin-h')
    }
  }, [])
  return (
    <div className="stats-pin" ref={ref}>
      {children}
    </div>
  )
}

/** One block of the grid. `wide` spans both columns. Shared with the usage page. */
export function Panel({ title, wide, children }: { title: string; wide?: boolean; children: ReactNode }) {
  return (
    <section className={wide ? 'stats-block wide' : 'stats-block'}>
      <div className="stats-block-head">{title}</div>
      {children}
    </section>
  )
}

/**
 * A row that can be marked, reachable without a mouse.
 *
 * `tabIndex` and the key handler, but no `role="button"`: the row is a table line
 * with three cells in it, and the role would claim the whole strip is one control
 * rather than say which cell was picked. Enter and Space act as the click does.
 */
export function pick(act: () => void) {
  return {
    tabIndex: 0,
    onClick: act,
    onKeyDown: (ev: React.KeyboardEvent) => {
      if (ev.key === 'Enter' || ev.key === ' ') {
        ev.preventDefault()
        act()
      }
    },
  }
}
