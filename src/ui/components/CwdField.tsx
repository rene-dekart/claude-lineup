import { useState } from 'react'
import * as api from '../api.js'
import { useT } from '../i18n.js'

/**
 * The project path field. Click it and the Explorer folder dialog opens — the same
 * one the OS shows any app — and what comes back is the exact folder that was
 * picked.
 *
 * The dialog is the server's, not the browser's. Chrome has its own picker
 * (`showDirectoryPicker`), and it is useless here: a page may not learn where a
 * folder lives, only what it is called. A name would have to be searched for on
 * the disks, and with two folders of the same name — one project cloned beside its
 * copy — that search answers with the wrong path and says nothing. The system
 * dialog has no such limit: it is asked from the server process, which is outside
 * the browser sandbox, and it returns the path itself.
 *
 * The pencil switches to typing; a dialog on every stray click would be worse than
 * a field you have to unlock. Same control in the topbar, in a project profile and
 * in the catalog window.
 */
interface Props {
  value: string
  onChange: (v: string) => void
  onStatus: (s: string) => void
  onBusy: (b: boolean) => void
  placeholder?: string
  /** Cap for the field; the row's own flex does the rest. */
  width?: number
}

export function CwdField({ value, onChange, onStatus, onBusy, placeholder, width }: Props) {
  const t = useT()
  const [edit, setEdit] = useState(false)
  const [pending, setPending] = useState(false)

  const pick = async () => {
    // One dialog per click would otherwise pile up: every click spawns a picker.
    if (pending) return
    setPending(true)
    onBusy(true)
    try {
      // Opens where the field already points, so the walk to a neighbouring
      // project starts from where the human last was.
      const r = await api.pickDir(value || undefined)
      if (r.path) onChange(r.path)
      else if (r.error) onStatus(r.error)
    } catch (e) {
      onStatus((e as Error).message)
    } finally {
      onBusy(false)
      setPending(false)
    }
  }

  return (
    <>
      <input
        className="input"
        style={{ cursor: edit ? 'text' : 'pointer', ...(width ? { maxWidth: width } : {}) }}
        placeholder={placeholder ?? t('ui.cwdPlaceholder')}
        title={edit ? t('ui.cwdType') : t('ui.cwdPick')}
        value={value}
        readOnly={!edit}
        onClick={() => !edit && void pick()}
        onKeyDown={(e) => {
          if (!edit && (e.key === 'Enter' || e.key === ' ')) {
            e.preventDefault()
            void pick()
          }
        }}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        className="btn icon"
        title={edit ? t('ui.cwdPick') : t('ui.cwdType')}
        onClick={() => setEdit((v) => !v)}
      >
        {edit ? '📁' : '✎'}
      </button>
    </>
  )
}
