import { useEffect, useState } from 'react'
import type { RestorePoint } from '../../shared/types.js'
import * as api from '../api.js'
import { useT } from '../i18n.js'

/**
 * The write history: one row per apply, each one rollable back.
 *
 * A restore point is the whole apply, not a file — restoring one file of a
 * write leaves the project half-turned-back, so a row is the unit and the button
 * is all-or-nothing. A rollback refuses when a file has been edited since, and
 * the server says which one.
 */
interface Props {
  busy: boolean
  onStatus: (s: string) => void
  /** Roll one point back. Owned by App: the statusbar has the same button. */
  onRestore: (id: string) => Promise<void>
  /** Points thrown away — App retires the statusbar's undo if it named one. */
  onForget: (ids: string[]) => void
  onClose: () => void
}

export function BackupsOverlay({ busy, onStatus, onRestore, onForget, onClose }: Props) {
  const t = useT()
  const [points, setPoints] = useState<RestorePoint[] | null>(null)
  /** Clearing everything is the one button here that cannot be taken back, so
   *  the first click only arms it. */
  const [armed, setArmed] = useState(false)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  async function load() {
    try {
      const r = await api.listBackups()
      setPoints(r.points)
    } catch (e) {
      setPoints([])
      onStatus((e as Error).message)
    }
  }

  useEffect(() => {
    void load()
    // Re-reading is the only thing this window does on open; `load`/`onStatus`
    // are stable in practice and would loop the effect on a new identity each render.
  }, [])

  const undo = async (id: string) => {
    await onRestore(id)
    // The rollback left a point of its own; the list is one row longer now.
    await load()
  }

  const drop = async (id: string) => {
    try {
      await api.deleteBackup(id)
      onForget([id])
      onStatus(t('ui.historyDeleted', { id }))
    } catch (e) {
      onStatus((e as Error).message)
    }
    await load()
  }

  const clear = async () => {
    setArmed(false)
    try {
      const { cleared } = await api.clearBackups()
      onForget((points ?? []).map((p) => p.id))
      onStatus(t('ui.historyCleared', { n: cleared }))
    } catch (e) {
      onStatus((e as Error).message)
    }
    await load()
  }

  return (
    <>
      <div className="catalog view">
        <div className="catalog-head">
          <span className="page-title">{t('ui.historyTitle')}</span>
          <div className="grow" />
          {points?.length ? (
            <button
              className={armed ? 'btn armed' : 'btn'}
              disabled={busy}
              title={t('ui.historyClearTitle')}
              onClick={() => (armed ? void clear() : setArmed(true))}
              onBlur={() => setArmed(false)}
            >
              {t(armed ? 'ui.historyClearArmed' : 'ui.historyClear')}
            </button>
          ) : null}
        </div>

        <div className="catalog-body">
          <div className="catalog-list">
            {!points && <div className="empty">{t('cat.loading')}</div>}
            {points?.length === 0 && <div className="empty">{t('ui.historyEmpty')}</div>}

            {points?.map((p) => (
              <div className="row" key={p.id}>
                <div className="colname">
                  <span className="name">{new Date(p.ts).toLocaleString()}</span>
                  <div className="desc">
                    {p.id} · {p.mode === 'profile' ? t('ui.historyProfile') : p.mode}
                  </div>
                </div>
                <span className="src" title={p.files[0]?.path ?? p.cwd}>
                  {/* A profile point writes the app's own file, so its `cwd` is a
                      directory the user has never seen. What tells two of them
                      apart is which profile, and that is the file's name. */}
                  {p.mode === 'profile'
                    ? (p.files[0]?.path.split(/[\\/]/).pop() ?? '')
                    : p.cwd}
                </span>
                <span className="chip">{t('ui.historyFiles', { n: p.files.length })}</span>
                <button
                  className="btn"
                  disabled={busy}
                  title={t('ui.undo')}
                  onClick={() => void undo(p.id)}
                >
                  {t('ui.undo')}
                </button>
                {/* One row, one point — the files it holds copies of stay where
                    they are, so this throws away a way back, not the work. */}
                <button
                  className="btn icon"
                  disabled={busy}
                  title={t('ui.historyDelete')}
                  onClick={() => void drop(p.id)}
                >
                  ✕
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  )
}
