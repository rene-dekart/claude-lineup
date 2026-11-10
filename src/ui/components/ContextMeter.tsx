import { useEffect, useRef, useState } from 'react'
import type { Inventory, Profile } from '../../shared/types.js'
import { contextTotal } from '../context-total.js'
import { useT } from '../i18n.js'

/** Claude Code prints `41.6k` and `1m`. The meter prints the same shape. */
function k(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n)
}

/**
 * The context figure, in the topbar, where a click on any row is watched
 * changing it.
 *
 * Two scales in one widget, because there are two questions and one number
 * cannot answer both.
 *
 * The bar is drawn against what the whole inventory would load — everything on,
 * plus the background. Its filled part is what this profile loads and its
 * hatched tail is what the profile leaves behind, so the tail *is* the saving
 * and every toggle moves the edge between them. Drawn against the window
 * instead, the bar would be a hairline: a 1M window turns a 6k server into a
 * sub-pixel step, and a change nobody can see is not a display of the change.
 *
 * The digits are the window figure, which is Claude Code's own — `41.6k/1M (4%)`.
 * That denominator comes from the project's `/context` report; without one there
 * is no window and the digits stand alone rather than against an invented one.
 */
export function ContextMeter({
  inventory,
  profile,
}: {
  inventory: Inventory | null
  profile: Profile
}) {
  const t = useT()
  const total = inventory ? contextTotal(inventory, profile) : null
  const tokens = total?.tokens ?? 0
  const full = total?.full ?? 0
  const delta = total?.delta ?? 0
  const window = inventory?.report?.window

  // The click that changed the number should be visible as a change, not only as
  // a new number: a flash in the direction it moved, then back to plain.
  const [moved, setMoved] = useState<'up' | 'down' | null>(null)
  const was = useRef<number | null>(null)
  useEffect(() => {
    const prev = was.current
    was.current = tokens
    if (prev === null || prev === tokens) return
    setMoved(tokens > prev ? 'up' : 'down')
    const id = setTimeout(() => setMoved(null), 700)
    return () => clearTimeout(id)
  }, [tokens])

  if (!inventory || tokens <= 0) return null

  // The bar's scale: the inventory's own ceiling. Percent of it the profile keeps.
  const share = full > 0 ? Math.min(100, (tokens / full) * 100) : 0
  const tail = full > 0 ? Math.max(0, ((full - tokens) / full) * 100) : 0
  const pct = window ? Math.min(100, Math.round((tokens / window) * 100)) : undefined
  // Rows nobody measured are counted in the profile but add nothing to the
  // number, so it is a floor, not a figure — and says so.
  const partial = (total?.known ?? 0) < (total?.all ?? 0)

  return (
    <div
      className={`ctx${moved ? ` ${moved}` : ''}`}
      title={
        inventory.report
          ? t('prof.ctxTotal', { known: total?.known ?? 0, all: total?.all ?? 0 })
          : t('prof.ctxNoSession')
      }
    >
      <span className="ctx-label">{t('prof.ctxMeter')}</span>
      {full <= 0 ? null : (
        <span
          className="ctx-track"
          aria-hidden="true"
          title={t('prof.ctxDelta', { delta: k(delta) })}
        >
          <span className="ctx-fill" style={{ width: `${share}%` }} />
          {tail > 0 ? (
            <span className="ctx-ghost" style={{ left: `${share}%`, width: `${tail}%` }} />
          ) : null}
        </span>
      )}
      <span className="ctx-num mono">
        {partial ? '≈' : ''}
        {k(tokens)}
        {window ? `/${k(window)}` : ''}
      </span>
      {pct === undefined ? null : <span className="ctx-pct mono">{pct}%</span>}
      {delta > 0 ? (
        <span className="ctx-delta mono" title={t('prof.ctxDelta', { delta: k(delta) })}>
          −{k(delta)}
        </span>
      ) : null}
    </div>
  )
}
