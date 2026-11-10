import { useEffect, useRef, useState } from 'react'
import type { KbAddResult, KbGroup, KbHit, KbList, KbPageView, KbRow, PickResult } from '../../shared/types.js'
import * as api from '../api.js'
import { useT } from '../i18n.js'
import { Readme } from '../md.js'

/** The three lists the side panel switches between. Documents are files, the other two pages. */
const GROUPS = [
  { key: 'mod.kb.docs', of: 'docs' },
  { key: 'mod.kb.entities', of: 'entities' },
  { key: 'mod.kb.concepts', of: 'concepts' },
] as const satisfies readonly { key: string; of: KbGroup }[]

/** Rows per request. A list this size is not a list anybody reads to the end. */
const PAGE = 100

/** How narrow the list may be dragged before it stops being a list. */
const SIDE_MIN = 200
const SIDE_MAX = 720

const base = (p: string): string => (p.split('/').pop() ?? p).replace(/\.md$/i, '')

/**
 * What a reload restores, kept the way the app keeps `cc-cols` and `cc-rail`.
 *
 * Only the things the user placed: which list, which hundred of it, how wide the
 * split is, and the page they had open. The search box is not among them — it is
 * a question asked once, not a position.
 */
const read = (key: string): string | null => localStorage.getItem(key)

function readNum(key: string, min: number, max: number, fallback: number): number {
  const v = Number(localStorage.getItem(key))
  return Number.isFinite(v) && v >= min && v <= max ? v : fallback
}

const save = (key: string, value: string | number | null): void => {
  if (value === null) localStorage.removeItem(key)
  else localStorage.setItem(key, String(value))
}

/**
 * openkb: what the knowledge base holds, searching it, and adding to it.
 *
 * Everything on this page is read off the disk (see the server's `modules/openkb`)
 * — openkb 0.4.5 has no API and no UI to ask, and its wiki is ordinary markdown.
 * The one thing asked of the CLI is the one thing reading cannot do: compiling a new
 * document. Nothing here edits a page, because openkb's index is incremental and
 * breaks silently on a hand edit.
 *
 * A list arrives a hundred rows at a time and the screen holds exactly those — no
 * appending. Appending a page at a time would end with the whole list in the DOM,
 * which is the thing paging is here to prevent.
 */
export function OpenkbView() {
  const t = useT()
  const [list, setList] = useState<KbList | null>(null)
  const [group, setGroup] = useState<KbGroup>(() => {
    const v = read('cc-kb-group')
    return GROUPS.find((g) => g.of === v)?.of ?? 'docs'
  })
  const [offset, setOffset] = useState(() => {
    const v = Number(read('cc-kb-offset'))
    return Number.isInteger(v) && v >= 0 ? v : 0
  })
  const [tick, setTick] = useState(0)
  const [side, setSide] = useState(() => readNum('cc-kb-side', SIDE_MIN, SIDE_MAX, 340))
  const [page, setPage] = useState<KbPageView | null>(null)
  const [q, setQ] = useState('')
  const [hits, setHits] = useState<KbHit[] | null>(null)
  const [adding, setAdding] = useState(false)
  const [mode, setMode] = useState<'local' | 'url'>('local')
  const [picked, setPicked] = useState('')
  const [url, setUrl] = useState('')
  const [busy, setBusy] = useState('')
  const [note, setNote] = useState('')

  const drag = useRef<{ startX: number; from: number } | null>(null)

  const fail = (e: unknown): void => setNote((e as Error).message)

  useEffect(() => {
    void (async () => {
      try {
        const r = await api.modGet<{ list: KbList }>('openkb', 'list', {
          group,
          offset: String(offset),
          limit: String(PAGE),
        })
        // A restored offset can outlive the list it pointed into — the wiki only
        // grows, but a base someone emptied takes the hundred away with it, and an
        // empty screen reads as a broken page rather than as a position past the end.
        if (r.list.total > 0 && r.list.offset >= r.list.total) setOffset(0)
        setList(r.list)
      } catch (e) {
        fail(e)
      }
    })()
  }, [group, offset, tick])

  // The place, written down as it changes. Four keys, four effects' worth of state:
  // one effect keeps them honest together.
  useEffect(() => {
    save('cc-kb-group', group)
    save('cc-kb-offset', offset)
    save('cc-kb-side', side)
  }, [group, offset, side])

  // The page that was open. Read once at mount: the effect that saves it runs on the
  // same pass and would have nothing to save yet.
  const wasOpen = useRef(read('cc-kb-page'))
  useEffect(() => {
    const p = wasOpen.current
    if (!p) return
    void (async () => {
      try {
        setPage((await api.modGet<{ page: KbPageView }>('openkb', 'page', { path: p })).page)
      } catch {
        // Gone from the wiki since last time: no page, and no note — the user did
        // not ask for anything just now.
        save('cc-kb-page', null)
      }
    })()
  }, [])

  useEffect(() => {
    save('cc-kb-page', page?.path ?? null)
  }, [page])

  const open = async (p: string): Promise<void> => {
    try {
      setPage((await api.modGet<{ page: KbPageView }>('openkb', 'page', { path: p })).page)
      setNote('')
    } catch (e) {
      fail(e)
    }
  }

  const search = async (): Promise<void> => {
    if (!q.trim()) {
      setHits(null)
      return
    }
    try {
      setHits((await api.modGet<{ hits: KbHit[] }>('openkb', 'search', { q })).hits)
    } catch (e) {
      fail(e)
    }
  }

  /** The desktop's picker, on the machine the server runs on: a real path, not a copy. */
  const browse = async (kind: 'file' | 'dir'): Promise<void> => {
    try {
      const r = await api.modPost<PickResult>('openkb', 'browse', { kind })
      if (r.error) setNote(r.error)
      else if (r.path) {
        setPicked(r.path)
        setNote('')
      }
    } catch (e) {
      fail(e)
    }
  }

  /**
   * Compiling a document takes minutes and the answer is the tool's, not ours: the
   * exit code and its own last line are what is shown. Nothing is summarised into a
   * nicer sentence — a paraphrase of `openkb` would be this app guessing.
   */
  const add = async (target: string): Promise<void> => {
    setBusy('add')
    setNote(t('mod.kb.adding'))
    try {
      const r = await api.modPost<KbAddResult>('openkb', 'add', { arg: target })
      const said = (r.stderr || r.stdout).trim().split(/\r?\n/).filter(Boolean).pop() ?? ''
      setNote(
        `${t(r.code === 0 ? 'mod.kb.addOk' : 'mod.kb.addFail', { code: r.code })}${said ? ` — ${said}` : ''}`,
      )
      if (r.code === 0) {
        // The dialog stays open on purpose: the exit code and the tool's own line are
        // the result, and closing on success would take the answer away with it.
        setPicked('')
        setUrl('')
        setTick((n) => n + 1)
      }
    } catch (e) {
      fail(e)
    } finally {
      setBusy('')
    }
  }

  // Pointer capture keeps the drag alive when the cursor outruns the 4px handle.
  const startDrag = (e: React.PointerEvent<HTMLDivElement>): void => {
    drag.current = { startX: e.clientX, from: side }
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {
      /* best-effort, as in App: without it the drag still tracks over the handle */
    }
  }

  const moveDrag = (e: React.PointerEvent<HTMLDivElement>): void => {
    const d = drag.current
    if (!d) return
    setSide(Math.max(SIDE_MIN, Math.min(SIDE_MAX, d.from + (e.clientX - d.startX))))
  }

  const endDrag = (e: React.PointerEvent<HTMLDivElement>): void => {
    drag.current = null
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId)
  }

  const counts = list?.counts
  const total = list?.total ?? 0
  const from = total === 0 ? 0 : offset + 1
  const to = offset + (list?.rows.length ?? 0)
  const target = mode === 'local' ? picked : url.trim()

  /** One row of whichever list is showing: a document is a file, the other two are pages. */
  const rowOf = (row: KbRow) => {
    // `raw/` is flat, so a document's name is its path — and the text behind it is the
    // document as it was added, before openkb compiled it into the wiki.
    const doc = 'name' in row
    const path = doc ? `raw/${row.name}` : row.path
    const links = doc ? { in: row.links, out: 0 } : { in: row.in, out: row.out }
    return (
      <div
        className="row click"
        key={path}
        role="button"
        tabIndex={0}
        onClick={() => void open(path)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') void open(path)
        }}
      >
        <span className="colname">
          <span className="name">{doc ? row.name : base(row.path)}</span>
          <span className="src">{t('mod.kb.bytes', { n: row.bytes })}</span>
        </span>
        <span className="chip">{t('mod.kb.links', links)}</span>
      </div>
    )
  }

  return (
    <div className="page">
      <div className="page-head">
        <span className="page-title">{t('mod.kb.title')}</span>
        {counts ? (
          <span className="tiny">
            {t('mod.kb.counts', {
              raw: counts.raw,
              wiki: counts.wiki,
              summaries: counts.summaries,
              sources: counts.sources,
            })}
          </span>
        ) : null}
        <div className="grow" />
        <button className="btn primary" onClick={() => setAdding(true)}>
          {t('mod.kb.add')}
        </button>
        <button className="btn" onClick={() => setTick((n) => n + 1)}>
          {t('mod.kb.refresh')}
        </button>
      </div>

      {/* Two ways in, one dialog: a path comes from the desktop's picker, a link is
          typed. Neither can do the other's job — a picker cannot choose a URL. */}
      {adding ? (
        <div className="scrim" onClick={() => setAdding(false)}>
          <div
            className="palette kb-modal"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key === 'Escape') setAdding(false)
            }}
          >
            <div className="kb-modal-head">
              <span className="page-title">{t('mod.kb.addTitle')}</span>
              <div className="grow" />
              <button className="btn" onClick={() => setAdding(false)}>
                {t('mod.kb.cancel')}
              </button>
            </div>
            <div className="kb-modal-body">
              <div className="seg kb-src">
                {/* `autoFocus`, not a ref: the segmented control is the first thing
                    the dialog offers and the keyboard has to land somewhere. */}
                <button autoFocus data-on={mode === 'local'} onClick={() => setMode('local')}>
                  {t('mod.kb.srcLocal')}
                </button>
                <button data-on={mode === 'url'} onClick={() => setMode('url')}>
                  {t('mod.kb.srcUrl')}
                </button>
              </div>
              {mode === 'local' ? (
                <div className="kb-pick">
                  <button className="btn" onClick={() => void browse('file')}>
                    {t('mod.kb.pickFile')}
                  </button>
                  <button className="btn" onClick={() => void browse('dir')}>
                    {t('mod.kb.pickDir')}
                  </button>
                  <span className={`path${picked ? '' : ' src'}`}>
                    {picked || t('mod.kb.nothingPicked')}
                  </span>
                </div>
              ) : (
                <input
                  className="input"
                  value={url}
                  placeholder={t('mod.kb.urlPh')}
                  aria-label={t('mod.kb.srcUrl')}
                  onChange={(e) => setUrl(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && target) void add(target)
                  }}
                />
              )}
              {note ? <div className="note">{note}</div> : null}
            </div>
            <div className="kb-modal-foot">
              <button
                className="btn primary"
                disabled={busy === 'add' || !target}
                onClick={() => void add(target)}
              >
                {busy === 'add' ? t('mod.kb.adding') : t('mod.kb.add')}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <div className="page-body kb-body">
        {/* The dialog carries its own copy: a failure that arrives while it is open
            belongs beside the button that caused it, not behind the scrim. */}
        {note && !adding ? <div className="note">{note}</div> : null}
        {list && !list.root ? (
          <div className="empty">{t('mod.kb.noRoot')}</div>
        ) : (
          <div
            className="kb"
            style={{ gridTemplateColumns: `${side}px 4px minmax(0, 1fr)` }}
          >
            <div className="kb-side">
              {/* Substring search over the whole wiki: the list below takes over
                  while there are hits, so an empty box is the way back to it. */}
              <div className="kb-search">
                <input
                  className="input"
                  value={q}
                  placeholder={t('mod.kb.searchPh')}
                  aria-label={t('mod.kb.search')}
                  onChange={(e) => {
                    setQ(e.target.value)
                    if (!e.target.value.trim()) setHits(null)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') void search()
                  }}
                />
                <button className="btn" disabled={!q.trim()} onClick={() => void search()}>
                  {t('mod.kb.search')}
                </button>
              </div>
              {hits ? (
                hits.length === 0 ? (
                  <div className="empty">{t('mod.kb.noHits')}</div>
                ) : (
                  <>
                    <div className="sub-title">{t('mod.kb.hits', { n: hits.length })}</div>
                    {hits.map((h) => (
                      <div
                        className="row click"
                        key={`${h.path}:${h.line}`}
                        role="button"
                        tabIndex={0}
                        onClick={() => void open(h.path)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') void open(h.path)
                        }}
                      >
                        <span className="colname">
                          <span className="name">{h.text}</span>
                          <span className="src">
                            {h.path}:{h.line}
                          </span>
                        </span>
                      </div>
                    ))}
                  </>
                )
              ) : (
                <>
                  <div className="kb-groups seg">
                    {GROUPS.map((g) => (
                      <button
                        key={g.of}
                        data-on={group === g.of}
                        onClick={() => {
                          // A different list starts at its own beginning: an offset
                          // carried over would land the reader past the end of a
                          // shorter one and show an empty screen.
                          setGroup(g.of)
                          setOffset(0)
                        }}
                      >
                        {t(g.key)}
                      </button>
                    ))}
                  </div>
                  <div className="kb-pager">
                    <button
                      className="btn"
                      disabled={offset <= 0}
                      aria-label={t('mod.kb.prev')}
                      onClick={() => setOffset(Math.max(0, offset - PAGE))}
                    >
                      ‹
                    </button>
                    <span className="tiny">{t('mod.kb.pageRange', { from, to, total })}</span>
                    <button
                      className="btn"
                      disabled={to >= total}
                      aria-label={t('mod.kb.next')}
                      onClick={() => setOffset(offset + PAGE)}
                    >
                      ›
                    </button>
                  </div>
                  {(list?.rows ?? []).map(rowOf)}
                </>
              )}
            </div>

            {/* The divider is the drag handle, exactly as between the panes of the app. */}
            <div
              className="handle"
              role="separator"
              aria-orientation="vertical"
              tabIndex={0}
              title={t('ui.resize')}
              aria-label={t('ui.resize')}
              aria-valuenow={side}
              aria-valuemin={SIDE_MIN}
              aria-valuemax={SIDE_MAX}
              onPointerDown={startDrag}
              onPointerMove={moveDrag}
              onPointerUp={endDrag}
              onPointerCancel={endDrag}
              onKeyDown={(e) => {
                if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return
                e.preventDefault()
                const step = e.key === 'ArrowRight' ? 16 : -16
                setSide(Math.max(SIDE_MIN, Math.min(SIDE_MAX, side + step)))
              }}
            />

            <div className="kb-page">
              {page ? (
                <>
                  <div className="kb-page-head">
                    <span className="kb-page-name">{base(page.path)}</span>
                    <span className="src">{page.path}</span>
                    <div className="grow" />
                    <span className="chip">{t('mod.kb.bytes', { n: page.bytes })}</span>
                    <span className="chip">
                      {t('mod.kb.links', { in: page.in.length, out: page.out.length })}
                    </span>
                  </div>
                  <Readme
                    text={page.text}
                    wiki={{ links: page.links, open: (p) => void open(p) }}
                  />
                </>
              ) : (
                <div className="empty">{t('mod.kb.pick')}</div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
