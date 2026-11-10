import { useEffect, useMemo, useRef, useState } from 'react'
import { MOD, useT } from '../i18n.js'

/**
 * One row of the ⌘K palette. The palette itself knows nothing about MCP,
 * plugins or profiles — whoever opens it hands over a flat list of things that
 * can be done, and each entry already carries the action that does it.
 */
export interface PaletteItem {
  key: string
  /** Kind label, already translated. */
  kind: string
  label: string
  /** Right-hand hint: scope, mode, cost. */
  note?: string
  run: () => void
}

interface Props {
  items: PaletteItem[]
  onClose: () => void
  /** Modifier+⏎ — applies the profile, the one action that is not a row. */
  onApply: () => void
}

export function CommandPalette({ items, onClose, onApply }: Props) {
  const t = useT()
  const [q, setQ] = useState('')
  const [sel, setSel] = useState(0)
  const input = useRef<HTMLInputElement>(null)

  useEffect(() => input.current?.focus(), [])

  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase()
    return needle ? items.filter((i) => i.label.toLowerCase().includes(needle)) : items
  }, [items, q])

  // The list shrinks as you type; the highlight must not end up past its end.
  const at = Math.min(sel, Math.max(0, shown.length - 1))

  const pick = (i: number) => {
    const item = shown[i]
    if (!item) return
    item.run()
    onClose()
  }

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const step = e.key === 'ArrowDown' ? 1 : -1
      setSel(Math.min(Math.max(at + step, 0), shown.length - 1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (e.metaKey || e.ctrlKey) {
        onApply()
        onClose()
      } else {
        pick(at)
      }
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  return (
    <div className="scrim" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <input
          ref={input}
          className="palette-input"
          placeholder={t('ui.searchPlaceholder')}
          value={q}
          onChange={(e) => {
            setQ(e.target.value)
            setSel(0)
          }}
          onKeyDown={onKey}
        />
        <div className="palette-list">
          {shown.length === 0 ? (
            <div className="empty">{t('ui.searchEmpty')}</div>
          ) : (
            shown.map((item, i) => (
              <div
                key={item.key}
                className="palette-row"
                data-sel={i === at}
                onMouseEnter={() => setSel(i)}
                onClick={() => pick(i)}
              >
                <span className="kind">{item.kind}</span>
                <span className="name">{item.label}</span>
                {item.note ? <span className="chip">{item.note}</span> : null}
              </div>
            ))
          )}
        </div>
        <div className="palette-foot">
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> {t('ui.hintNav')}
          </span>
          <span>
            <kbd>⏎</kbd> {t('ui.hintOpen')}
          </span>
          <span>
            <kbd>{MOD}⏎</kbd> {t('ui.hintApply')}
          </span>
          <span>
            <kbd>⎋</kbd> {t('ui.hintClose')}
          </span>
        </div>
      </div>
    </div>
  )
}
