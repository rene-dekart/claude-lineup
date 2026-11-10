import { useEffect, useState } from 'react'
import type {
  Card as CardData,
  McpRegistry,
  McpRegistryRow,
  RemoteCatalog,
  RemoteListing,
  RemotePlan,
} from '../../shared/types.js'
import * as api from '../api.js'
import { useT } from '../i18n.js'
import { Card } from './Card.js'
import { CwdField } from './CwdField.js'

/**
 * One catalog window, one registry. Which one is a prop, because the two are
 * destinations in the rail rather than tabs of a single window.
 *
 * The external skillselion index (not paged — their API returns no total per
 * query, so it grows a page at a time and says so), and the official MCP registry
 * (paged by cursor, and read-only: nothing installs from it, so its rows have no
 * install button and the scope picker is not shown for it). Both windows name the
 * host they came from — the base urls are env-overridable, so a hardcoded label
 * would lie about where the rows came from.
 */
interface PendingInstall {
  id: string
  scope: string
  command: string
  sha256: string
  remote?: { repo: string; name: string }
}

interface Props {
  /** Which registry this window is. The rail picks it, not a control in here. */
  source: 'plugins' | 'mcp'
  cwd: string
  /** The catalog covers the topbar, so the project path has to be settable here too. */
  onCwd: (v: string) => void
  busy: boolean
  onBusy: (b: boolean) => void
  onStatus: (s: string) => void
  onInstalled: (id?: string) => Promise<void>
  onClose: () => void
}

export function CatalogOverlay({ source, cwd, onCwd, busy, onBusy, onStatus, onInstalled, onClose }: Props) {
  const t = useT()
  const [q, setQ] = useState('')
  const [dq, setDq] = useState('')
  const [sort, setSort] = useState('stars')
  const [scope, setScope] = useState('user')
  const [remote, setRemote] = useState<RemoteCatalog | null>(null)
  /** The last page, for its cursor and notes; and everything loaded so far. */
  const [mcp, setMcp] = useState<McpRegistry | null>(null)
  const [mcpRows, setMcpRows] = useState<McpRegistryRow[]>([])
  const [netLimit, setNetLimit] = useState(30)
  const [loading, setLoading] = useState(true)
  const [plan, setPlan] = useState<RemotePlan | null>(null)
  const [pending, setPending] = useState<PendingInstall | null>(null)
  const [card, setCard] = useState<{
    kind: CardData['kind']
    name: string
    repo?: string
    description?: string
    version?: string
    transport?: string
    file?: string
    scope?: string
    origin?: string
    installed?: boolean
    installable?: boolean
  } | null>(null)

  // Escape leaves for the workspace — unless a card is open over it, which closes first.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      if (card) setCard(null)
      else onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [card, onClose])

  // A search is a network round trip and the registry takes seconds: type, then
  // one request — not one per keystroke.
  useEffect(() => {
    const timer = setTimeout(() => {
      setDq(q)
      setNetLimit(30)
    }, 250)
    return () => clearTimeout(timer)
  }, [q])

  useEffect(() => {
    let dead = false
    if (source !== 'plugins') return
    setLoading(true)
    void api
      .getRemote(dq, sort, '', netLimit)
      .then((r) => !dead && setRemote(r))
      .catch((e: Error) => !dead && onStatus(t('ui.indexUnreadable', { message: e.message })))
      .finally(() => !dead && setLoading(false))
    return () => {
      dead = true
    }
  }, [dq, sort, netLimit, source])

  // Registry pages are cursor-based and append: a new query starts over at the
  // first page, "more" asks for the page after the last one.
  useEffect(() => {
    let dead = false
    if (source !== 'mcp') return
    setLoading(true)
    void api
      .getMcpRegistry(dq)
      .then((r) => {
        if (dead) return
        setMcp(r)
        setMcpRows(r.entries)
      })
      .catch((e: Error) => !dead && onStatus(t('ui.indexUnreadable', { message: e.message })))
      .finally(() => !dead && setLoading(false))
    return () => {
      dead = true
    }
  }, [dq, source])

  const moreMcp = async () => {
    if (!mcp?.nextCursor) return
    try {
      const r = await api.getMcpRegistry(dq, 30, mcp.nextCursor)
      setMcp(r)
      setMcpRows((prev) => [...prev, ...r.entries])
    } catch (e) {
      onStatus((e as Error).message)
    }
  }

  const askInstall = async (id: string, acceptCommand?: string) => {
    onBusy(true)
    onStatus(t('ui.installing', { id, scope }))
    try {
      const rt = pending?.remote
      if (rt) {
        const r = await api.runRemoteInstall({
          ...rt,
          scope,
          cwd,
          ...(acceptCommand ? { acceptCommand } : {}),
        })
        if (!r.ok) {
          onStatus(t('ui.installFailedId', { id, error: r.error ?? '' }))
          return
        }
        setPending(null)
        setPlan(null)
        onStatus(t('ui.installed', { name: plan?.pluginId ?? id }))
        await onInstalled(plan?.pluginId ?? id)
        return
      }
      const r = await api.installPlugin({
        id,
        scope,
        cwd,
        ...(acceptCommand ? { acceptCommand } : {}),
      })
      if (r.pending) {
        setPending({ id, scope, command: r.pending.command, sha256: r.pending.sha256 })
        onStatus(t('ui.pendingConfirm', { id }))
        return
      }
      if (!r.ok) {
        onStatus(
          t('ui.installFailedCode', {
            id,
            code: r.code ?? '—',
            output: (r.stderr || r.stdout).trim().slice(0, 400),
          }),
        )
        return
      }
      setPending(null)
      onStatus(t('ui.installedAndAdded', { id }))
      await onInstalled(id)
    } catch (e) {
      onStatus(t('ui.installRejected', { message: (e as Error).message }))
    } finally {
      onBusy(false)
    }
  }

  const doPlan = async (l: RemoteListing) => {
    onBusy(true)
    onStatus(t('ui.planBuilding', { repo: l.repo }))
    try {
      const p = await api.planRemoteInstall({
        repo: l.repo,
        name: l.name,
        ...(l.skillCommitSha ? { skillCommitSha: l.skillCommitSha } : {}),
        scope,
        cwd,
      })
      setPlan(p)
      onStatus(
        p.ok
          ? t('ui.planReady', { steps: p.steps.length })
          : t('ui.planIncomplete', { error: p.error ?? '' }),
      )
    } catch (e) {
      onStatus(t('ui.planFailed', { message: (e as Error).message }))
    } finally {
      onBusy(false)
    }
  }

  const runPlan = async () => {
    if (!plan) return
    onBusy(true)
    try {
      const name = plan.pluginId?.split('@')[0] ?? plan.repo
      const r = await api.runRemoteInstall({ repo: plan.repo, name, scope, cwd })
      if (r.pending) {
        setPending({
          id: plan.pluginId ?? plan.repo,
          scope,
          command: r.pending.command,
          sha256: r.pending.sha256,
          remote: { repo: plan.repo, name },
        })
        onStatus(t('ui.marketplacePending'))
        return
      }
      if (!r.ok) {
        onStatus(t('ui.installFailed', { error: r.error ?? t('ui.seeCommandOutput') }))
        return
      }
      setPlan(null)
      onStatus(t('ui.installed', { name: plan.pluginId ?? '' }))
      await onInstalled(plan.pluginId)
    } catch (e) {
      onStatus(t('ui.installRejected', { message: (e as Error).message }))
    } finally {
      onBusy(false)
    }
  }

  /** How many rows the table has. No rows is no table: a head over "the index returned
   *  nothing" would be a table of nothing. */
  const shown = source === 'plugins' ? (remote?.entries ?? []).length : mcpRows.length

  // `open` is optional: a row without one is plain text. Every row in both
  // registries has one — what it opens differs, not whether it opens. The cells
  // after the name are the row's, because the two registries do not carry the same
  // ones: the index has stars and a risk level, the registry has transports.
  const row = (e: {
    key: string
    name: string
    desc?: string
    open?: () => void
    cells: React.ReactNode[]
  }) => (
    <div className={e.open ? 'row click' : 'row'} key={e.key} onClick={e.open}>
      <div className="colname">
        <span className="name">{e.name}</span>
        {e.desc && <div className="desc">{e.desc}</div>}
      </div>
      {e.cells}
    </div>
  )

  /** Where the row came from, and the one way out of the app from here. Descriptions
   *  are other people's English and stay as written — the link is how you find out
   *  more, so it travels with the repository rather than standing on its own. */
  const repoCell = (repo?: string, link?: string) => (
    <span className="cat-repo" key="repo">
      <span className="cat-repo-name">{repo ?? ''}</span>
      {link && (
        <a
          className="rowlink"
          href={link}
          target="_blank"
          rel="noreferrer"
          title={t('cat.openRepo', { url: link })}
          onClick={(ev) => ev.stopPropagation()}
        >
          ↗
        </a>
      )}
    </span>
  )

  return (
    <>
      <div className="catalog view">
        <div className="catalog-head">
          <span className="page-title">{source === 'plugins' ? t('cat.plugins') : t('cat.mcp')}</span>
          {/* Where the rows came from. The base urls are env-overridable, so the
              host is the server's answer, never a literal written here. */}
          <span className="brand">
            {source === 'plugins'
              ? [`${remote?.source ?? ''}`, remote?.total ? String(remote.total) : '']
                  .filter(Boolean)
                  .join(' · ')
              : (mcp?.source ?? '')}
          </span>
          <input
            className="input"
            autoFocus
            title={t('cat.search')}
            placeholder={source === 'plugins' ? t('inv.searchPlugins') : t('inv.searchRegistry')}
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
          {source === 'plugins' && (
            <select className="input" value={sort} title={t('inv.sortTitle')} onChange={(e) => setSort(e.target.value)}>
              {[
                ['stars', 'inv.sortStars'],
                ['installs', 'inv.sortInstalls'],
                ['trust', 'inv.sortTrust'],
                ['name', 'inv.sortName'],
              ].map(([v, k]) => (
                <option key={v} value={v}>
                  {t(k as string)}
                </option>
              ))}
            </select>
          )}
          {/* Nothing installs from the registry, so there is no scope to pick. */}
          {source !== 'mcp' && (
            <select className="input" value={scope} title={t('inv.scopeTitle')} onChange={(e) => setScope(e.target.value)}>
              <option value="user">user</option>
              <option value="project">project</option>
              <option value="local">local</option>
            </select>
          )}
          {source !== 'mcp' && scope !== 'user' && (
            <CwdField value={cwd} onChange={onCwd} onStatus={onStatus} onBusy={onBusy} width={260} />
          )}
        </div>

        <div className="catalog-body">
          <div className="catalog-list cat-list">
            {loading && <div className="empty">{t('cat.loading')}</div>}

            {/* One table per registry: the head names the columns once and every row
                shares its grid, so a figure read down is a column and not a
                coincidence. The two sources do not carry the same columns — the index
                has ratings, the registry has transports — so the tables differ. */}
            {shown > 0 && (
              <div className={source === 'mcp' ? 'cat-table mcp' : 'cat-table'}>
                <div className="cat-head">
                  <span>{t('cat.colName')}</span>
                  <span>{t('cat.link')}</span>
                  {source === 'mcp' ? (
                    <span>{t('cat.transport')}</span>
                  ) : (
                    <span className="cat-num">{t('inv.sortStars')}</span>
                  )}
                  <span />
                </div>

                {source === 'plugins' &&
                  (remote?.entries ?? []).map((l) =>
                    row({
                      key: l.id,
                      name: l.name,
                      desc: l.description || l.summary,
                      // `owner/repo` as their API spells it — not a guessed link.
                      ...(l.repo ? { link: `https://github.com/${l.repo}` } : {}),
                      // Same card as an MCP row: the repository's readme, its
                      // description off the listing, the link out. Not installed, so
                      // the card offers no install of its own — the row's button
                      // builds the plan.
                      open: () =>
                        setCard({
                          kind: 'plugin',
                          name: `${l.name}@${l.repo}`,
                          repo: l.repo,
                          ...(l.description || l.summary
                            ? { description: l.description || l.summary }
                            : {}),
                          installable: false,
                        }),
                      cells: [
                        repoCell(l.repo, l.repo ? `https://github.com/${l.repo}` : undefined),
                        <span className="cat-num" key="stars">
                          {l.stars !== undefined ? `${l.stars}★` : ''}
                        </span>,
                        <span className="cat-act" key="act">
                          <button
                            className="btn"
                            disabled={busy}
                            title={t('inv.planTitle', { repo: l.repo })}
                            onClick={(ev) => {
                              ev.stopPropagation()
                              void doPlan(l)
                            }}
                          >
                            {t('inv.install')}
                          </button>
                        </span>,
                      ],
                    }),
                  )}

                {source === 'mcp' &&
                  mcpRows.map((s) =>
                    row({
                      key: s.name,
                      name: s.title,
                      desc: s.description,
                      // The card is the row's whole answer: the readme off its
                      // repository, the version and the transport the row already
                      // carried. Not installed, so nothing here is on disk.
                      open: () =>
                        setCard({
                          kind: 'mcp',
                          name: s.name,
                          ...(s.repo ? { repo: s.repo } : {}),
                          ...(s.description ? { description: s.description } : {}),
                          ...(s.version ? { version: s.version } : {}),
                          ...(s.transports.length ? { transport: s.transports.join(', ') } : {}),
                        }),
                      cells: [
                        repoCell(s.repo, s.repo ? `https://github.com/${s.repo}` : undefined),
                        <span className="cat-cell" key="tr">
                          {s.transports.map((c) => (
                            <span className="chip" key={c}>
                              {c}
                            </span>
                          ))}
                        </span>,
                        // Read-only: the row shows, the link opens the repository.
                        <span className="cat-act" key="act">
                          {s.secret && (
                            <span className="chip secret" title={t('inv.mcpSecretTitle')}>
                              {t('inv.mcpSecret')}
                            </span>
                          )}
                        </span>,
                      ],
                    }),
                  )}
              </div>
            )}

            {source === 'mcp' && mcp && mcpRows.length === 0 && (
              <div className="note">{t('inv.indexEmpty')}</div>
            )}

            {source === 'mcp' && mcp?.nextCursor && (
              <div className="row-gap cat-more">
                <button className="btn" onClick={() => void moreMcp()}>
                  {t('cat.more')}
                </button>
              </div>
            )}

            {source === 'plugins' && remote && remote.entries.length === 0 && !loading && (
              <div className="note">{dq ? t('inv.indexEmpty') : t('inv.indexEmptyBare')}</div>
            )}

            {source === 'plugins' && remote?.hasMore && (
              <div className="row-gap cat-more">
                <button className="btn" disabled={busy} onClick={() => setNetLimit((n) => n + 30)}>
                  {t('cat.more')}
                </button>
                <span className="muted">{t('inv.hasMore')}</span>
              </div>
            )}

            {(source === 'plugins' ? (remote?.notes ?? []) : (mcp?.notes ?? [])).map((n, i) => (
              <div className="note" key={i}>
                {n}
              </div>
            ))}
          </div>

          {card && (
            <Card
              kind={card.kind}
              name={card.name}
              cwd={cwd}
              {...(card.repo ? { repo: card.repo } : {})}
              {...(card.description ? { description: card.description } : {})}
              {...(card.version ? { version: card.version } : {})}
              {...(card.transport ? { transport: card.transport } : {})}
              {...(card.file ? { file: card.file } : {})}
              {...(card.scope ? { scope: card.scope } : {})}
              {...(card.origin ? { origin: card.origin } : {})}
              {...(card.installed !== undefined ? { installed: card.installed } : {})}
              installable={card.installable}
              busy={busy}
              onInstall={(name) => void askInstall(name)}
              onClose={() => setCard(null)}
            />
          )}
        </div>

        {(plan || pending) && (
          <div className="catalog-foot">
            {plan && (
              <div className="note warn">
                <div>
                  {t('inv.planHead')} <b>{plan.repo}</b> {t('inv.planRest')}
                </div>
                {plan.steps.map((s) => (
                  <pre className="cmd small" key={s.display}>
                    {s.display}
                  </pre>
                ))}
                {plan.pluginId && <div className="muted">{t('inv.pluginLabel', { id: plan.pluginId })}</div>}
                {plan.notes.map((n, i) => (
                  <div className="muted" key={i}>
                    {n}
                  </div>
                ))}
                {plan.error && <div className="muted">{plan.error}</div>}
                <div className="row-gap" style={{ marginTop: 6 }}>
                  <button
                    className="btn danger"
                    disabled={busy || !plan.ok}
                    title={plan.ok ? t('inv.runPlanTitle') : t('inv.planIncompleteTitle')}
                    onClick={() => void runPlan()}
                  >
                    {t('inv.runViaCli')}
                  </button>
                  <button className="btn" disabled={busy} onClick={() => setPlan(null)}>
                    {t('inv.cancel')}
                  </button>
                </div>
              </div>
            )}
            {pending && (
              <div className="note warn">
                <div>
                  {t('inv.marketHead')} <b>{pending.id}</b> {t('inv.marketRest')}
                </div>
                <pre className="cmd small">{pending.command}</pre>
                <div className="muted">sha256: {pending.sha256}</div>
                <div className="row-gap" style={{ marginTop: 6 }}>
                  <button className="btn danger" disabled={busy} onClick={() => void askInstall(pending.id, pending.sha256)}>
                    {t('inv.allowAndInstall')}
                  </button>
                  <button className="btn" disabled={busy} onClick={() => setPending(null)}>
                    {t('inv.cancel')}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

      </div>
    </>
  )
}
