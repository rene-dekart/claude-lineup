import { useEffect, useState } from 'react'
import type { ModuleProbeState, ModuleRow } from '../../shared/types.js'
import * as api from '../api.js'
import { useT } from '../i18n.js'

/** The tool column, per answer the machine gave — or has not given yet. */
const CHIP: Record<ModuleProbeState, string> = {
  found: 'mod.state.found',
  'not-found': 'mod.state.notFound',
  unknown: 'mod.state.unknown',
}

/**
 * The catalog: what this build carries, whether the tool is on this machine, and the
 * switch.
 *
 * Two facts per row and they are different facts: the tool is the machine's answer,
 * the switch is the human's. A module whose tool is missing cannot be switched on —
 * the button is disabled rather than hidden, because a row that offers nothing is
 * still a row that tells you what to install.
 *
 * This page is where the tool gets looked for, and the only page that asks. The list
 * arrives unprobed (a check starts a process per module, 4.1 s for openkb measured
 * here), so the rows are on screen at once and the column fills in behind them. The
 * button waits for the answer: switching a module on whose tool is absent is a
 * decision the row cannot honestly offer.
 */
export function ModulesPage({
  modules,
  onChange,
  onStatus,
}: {
  modules: ModuleRow[]
  onChange: (rows: ModuleRow[]) => void
  onStatus: (s: string) => void
}) {
  const t = useT()
  const [busy, setBusy] = useState('')
  /**
   * True until this page's own answer arrives. It only changes how a row that is
   * still `unknown` reads — a row the server has already answered keeps its answer,
   * so coming back to this page shows the cached one instead of flashing.
   */
  const [checking, setChecking] = useState(true)

  useEffect(() => {
    void api
      .getModules(true)
      .then((r) => onChange(r.modules))
      // A probe that cannot be asked for leaves the column as it was. Nothing else on
      // this page depends on it.
      .catch(() => {})
      .finally(() => setChecking(false))
  }, [onChange])

  const flip = async (m: ModuleRow): Promise<void> => {
    setBusy(m.id)
    try {
      onChange((await api.setModuleEnabled(m.id, !m.enabled)).modules)
    } catch (e) {
      onStatus((e as Error).message)
    } finally {
      setBusy('')
    }
  }

  return (
    <div className="page">
      <div className="page-head">
        <span className="page-title">{t('mod.head')}</span>
      </div>
      <div className="page-body">
        {/* An empty list before the answer is not "this build carries no modules" —
            it is a list that has not arrived. Saying the first when the second is
            true is the kind of lie a dash would be. */}
        {modules.length === 0 && checking ? (
          <div className="empty">{t('mod.state.checking')}</div>
        ) : modules.length === 0 ? (
          <div className="empty">{t('mod.empty')}</div>
        ) : (
          <div className="mod-table">
            <div className="mod-head">
              <span>{t('mod.col.module')}</span>
              <span>{t('mod.col.tool')}</span>
              <span />
              <span className="mod-act">{t('mod.col.action')}</span>
            </div>
            {modules.map((m) => {
              const found = m.probe.state === 'found'
              const chip = checking && m.probe.state === 'unknown' ? 'mod.state.checking' : CHIP[m.probe.state]
              return (
                <div className="row" key={m.id}>
                  <div className="mod-name">{t(m.titleKey)}</div>
                  <div className="mod-cell">
                    {/* `unknown` while the tool is absent, `measured` when it answered
                        — the same two chips the rest of the app uses for a number it
                        has and a number it does not. */}
                    <span
                      className={`chip ${found ? 'measured' : 'unknown'}`}
                      {...(found ? {} : m.probe.detail ? { title: m.probe.detail } : {})}
                    >
                      {t(chip)}
                    </span>
                    {m.probe.version ? <span className="chip">{m.probe.version}</span> : null}
                  </div>
                  <div className="mod-state">{t(m.enabled ? 'mod.on' : 'mod.off')}</div>
                  <div className="mod-act">
                    <button
                      className="btn"
                      disabled={busy === m.id || !found}
                      onClick={() => void flip(m)}
                    >
                      {t(m.enabled ? 'mod.disable' : 'mod.enable')}
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
