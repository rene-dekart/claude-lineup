import { useId } from 'react'
import type { McpMode, Profile, SkillMode } from '../../shared/types.js'
import { useT } from '../i18n.js'

/** A select option carrying dictionary keys, not finished copy. */
type Opt<T extends string> = { v: T; label: string; title?: string }

/**
 * The explanation of a field, kept out of the form until asked for. A click, not
 * a hover: the popover API gives the toggle, the light dismiss and the top layer
 * in one attribute, so nothing can clip it and there is no state to keep.
 *
 * Takes the finished text so the key stays a literal `t('…')` call — that is what
 * the dictionary test scans for, and a prop would slip past it.
 */
function Hint({ text }: { text: string }) {
  const id = useId()
  return (
    <>
      <button type="button" className="hint" popoverTarget={id} aria-label={text}>
        !
      </button>
      <span id={id} popover="auto" className="hint-pop">
        {text}
      </span>
    </>
  )
}

const MCP_OPTIONS: Opt<McpMode>[] = [
  { v: 'all', label: 'prof.all' },
  { v: 'only', label: 'prof.only', title: 'prof.mcpOnlyTitle' },
  { v: 'disable', label: 'prof.none', title: 'prof.mcpDisableTitle' },
]

const SKILL_OPTIONS: Opt<SkillMode>[] = [
  { v: 'all', label: 'prof.all' },
  { v: 'none', label: 'prof.none', title: 'prof.skillsNoneTitle' },
]

interface Props {
  draft: Profile
  profiles: Profile[]
  onChange: (p: Partial<Profile>) => void
  onNew: () => void
  onLoad: (id: string) => void
  onDelete: (id: string) => void
  onSave: () => void
  busy: boolean
}

function Seg<T extends string>({
  value,
  options,
  onPick,
}: {
  value: T
  options: Array<{ v: T; label: string; title?: string }>
  onPick: (v: T) => void
}) {
  const t = useT()
  return (
    <div className="seg">
      {options.map((o) => (
        <button
          key={o.v}
          data-on={o.v === value}
          title={o.title ? t(o.title) : undefined}
          onClick={() => onPick(o.v)}
        >
          {t(o.label)}
        </button>
      ))}
    </div>
  )
}

export function ProfilePane({
  draft,
  profiles,
  onChange,
  onNew,
  onLoad,
  onDelete,
  onSave,
  busy,
}: Props) {
  const t = useT()

  return (
    <section className="pane">
      <div className="pane-head">
        <span>{t('prof.title')}</span>
      </div>

      <div className="pane-body">
        <div className="field">
          <label>
            {t('prof.load')}
            <Hint text={t('prof.loadHint')} />
          </label>
          <select
            className="input"
            value={profiles.some((p) => p.id === draft.id) ? draft.id : ''}
            onChange={(e) => e.target.value && onLoad(e.target.value)}
          >
            <option value="">{t('prof.selectPlaceholder')}</option>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.title} ({p.id})
              </option>
            ))}
          </select>
          <button className="btn" onClick={onNew}>
            {t('prof.new')}
          </button>
        </div>

        <div className="field">
          <label>
            id
            <Hint text={t('prof.idHint')} />
          </label>
          <input
            className="input"
            value={draft.id}
            onChange={(e) => onChange({ id: e.target.value })}
          />
        </div>

        <div className="field">
          <label>
            {t('prof.titleField')}
            <Hint text={t('prof.titleHint')} />
          </label>
          <input
            className="input"
            value={draft.title}
            onChange={(e) => onChange({ title: e.target.value })}
          />
        </div>

        <div className="field">
          <label>
            MCP
            <Hint text={t('prof.mcpHint')} />
          </label>
          <Seg
            value={draft.mcp.mode}
            options={MCP_OPTIONS}
            onPick={(v) => onChange({ mcp: { ...draft.mcp, mode: v } })}
          />
        </div>

        {draft.mcp.mode === 'only' && (
          <div className="field">
            <label />
            <span className="muted mono">
              {draft.mcp.servers.length
                ? draft.mcp.servers.join(', ')
                : t('prof.mcpPickServers')}
            </span>
          </div>
        )}

        <div className="field">
          <label>
            {t('prof.skills')}
            <Hint text={t('prof.skillsHint')} />
          </label>
          <Seg
            value={draft.skills.mode}
            options={SKILL_OPTIONS}
            onPick={(v) => onChange({ skills: { ...draft.skills, mode: v } })}
          />
        </div>

        <div className="field">
          <label>
            {t('prof.hooks')}
            <Hint text={t('prof.hooksHint')} />
          </label>
          <label className="mono" style={{ display: 'flex', gap: 6 }}>
            <input
              type="checkbox"
              checked={draft.hooks.disableAll}
              onChange={(e) => onChange({ hooks: { disableAll: e.target.checked } })}
            />
            {t('prof.hooksAll')}
          </label>
        </div>

        <div className="field">
          <label>
            CLAUDE.md
            <Hint text={t('prof.memoryHint')} />
          </label>
          <label className="mono" style={{ display: 'flex', gap: 6 }}>
            <input
              type="checkbox"
              checked={draft.memory.autoDiscover}
              onChange={(e) => onChange({ memory: { autoDiscover: e.target.checked } })}
            />
            {t('prof.memoryAuto')}
          </label>
        </div>

      </div>

      <div className="pane-foot row-gap">
        <button className="btn primary" onClick={onSave} disabled={busy || !draft.id}>
          {t('prof.save')}
        </button>
        <button
          className="btn danger"
          disabled={busy || !profiles.some((p) => p.id === draft.id)}
          onClick={() => onDelete(draft.id)}
        >
          {t('prof.delete')}
        </button>
      </div>
    </section>
  )
}
