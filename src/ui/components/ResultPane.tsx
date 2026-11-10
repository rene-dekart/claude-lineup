import { useEffect, useState } from 'react'
import type { ConfigFile, Profile, Resolution } from '../../shared/types.js'
import { settingsPatch } from '../../shared/settings-patch.js'
import { diffFolded, type DiffLine } from '../diff.js'
import { useT } from '../i18n.js'
import * as api from '../api.js'

interface Props {
  /** The profile as it stands: the settings below are read off it directly. */
  profile: Profile
  result: { resolution: Resolution } | null
  /**
   * The draft has been edited since this resolution was made, and the rebuild is
   * on its way. The diff stays on screen — a pane emptied on every click is worse
   * — but Apply waits: it must not write under a diff that no longer describes
   * what it is about to write.
   */
  stale: boolean
  busy: boolean
  cwd: string
  onResolve: () => void
  onApply: () => void
}

export function ResultPane({ profile, result, stale, busy, cwd, onResolve, onApply }: Props) {
  const t = useT()
  const resolution = result?.resolution ?? null

  /**
   * The pane reads the machine three ways and the switch is the whole difference.
   * «Профиль» is the draft — the patch, the diff, the two buttons that write it.
   * «Полная конфигурация» is the machine: every file that shapes the project, as
   * it lies, nothing to press. «claude.json» is one of those files on its own —
   * the machine-wide half of it, which the other reading only ever shows the
   * project slice of.
   *
   * The files are read on the click, not on load: the inventory is already the
   * slow request this screen makes, and a project's four files are not worth a
   * second one until somebody asks for them.
   */
  const [view, setView] = useState<'draft' | 'full' | 'claude'>('draft')
  const [files, setFiles] = useState<ConfigFile[] | null>(null)
  const [configError, setConfigError] = useState<string | null>(null)

  useEffect(() => {
    if (view === 'draft' || !cwd.trim()) return
    let live = true
    setFiles(null)
    setConfigError(null)
    // One file reads as a list of one, so everything below — loading, error,
    // empty, the printing — is written once for both readings.
    const load =
      view === 'full'
        ? api.getConfigFiles(cwd).then((r) => r.files)
        : api.getClaudeJson(cwd).then((r) => (r.file ? [r.file] : []))
    load.then((f) => live && setFiles(f)).catch((e: Error) => live && setConfigError(e.message))
    return () => {
      live = false
    }
  }, [view, cwd])

  /**
   * The block at the top is the patch, and it is shown only while the patch is
   * the whole answer.
   *
   * The file that gets written is not the patch: the server merges it into what
   * the file already holds. Once that has happened the diff below *is* the file —
   * every line of it, the unchanged ones muted — so printing the merged JSON
   * above the diff put one configuration on screen twice. The patch keeps its
   * place only before the first build, where it is what the profile asks for
   * rather than what the file will hold, and it is labelled as a patch so it is
   * not read as the file.
   */
  const written = resolution?.changes.find((c) => /settings\.json$/.test(c.path))

  // A missing `~/.claude.json` is not "no settings files" — it is that one file,
  // and the two readings are two different absences.
  const empty = view === 'claude' ? t('ui.claudeJsonEmpty') : t('ui.configEmpty')

  return (
    <section className="pane">
      <div className="pane-head">
        <span className="seg">
          <button data-on={view === 'draft'} onClick={() => setView('draft')}>
            {t('ui.viewDraft')}
          </button>
          <button data-on={view === 'full'} onClick={() => setView('full')}>
            {t('ui.viewFull')}
          </button>
          <button
            data-on={view === 'claude'}
            title={t('ui.claudeJsonHint')}
            onClick={() => setView('claude')}
          >
            {t('ui.viewClaudeJson')}
          </button>
        </span>
        <span style={{ flex: 1 }} />
      </div>

      <div className="pane-body">
        {view === 'draft' ? (
          <>
          {written ? null : (
            <>
              <div className="group-title">
                <span>{t('ui.patchDraft')}</span>
              </div>
              <pre className="json">{JSON.stringify(settingsPatch(profile), null, 2)}</pre>
            </>
          )}

          {resolution?.changes.map((c) => {
            // A file that does not exist yet has no lines to compare against —
            // diffing it as an empty file would print one phantom removal.
            const lines: DiffLine[] =
              c.before === null
                ? (c.after ?? '').split('\n').map((text) => ({ type: 'add', text }))
                : diffFolded(c.before, c.after ?? '')
            const changed = lines.filter((l) => l.type === 'add' || l.type === 'del').length
            return (
              <div key={c.path}>
                <div className="group-title">
                  <span className="mono" style={{ textTransform: 'none', letterSpacing: 0 }}>
                    {c.path}
                  </span>
                  <span>{t('ui.lines', { n: changed })}</span>
                </div>
                {/* The whole file, unchanged lines included: a diff with the rest of
                    the text left in reads as a file, one with only the edits left in
                    reads as a puzzle. The unchanged lines stay muted so the colored
                    ones carry. Only the big shared file folds its untouched middle —
                    counting it, never hiding it. */}
                <div className="diff">
                  {lines.map((l, i) => (
                    <div key={i} className={l.type}>
                      {l.type === 'add' ? '+ ' : l.type === 'del' ? '- ' : '  '}
                      {l.type === 'fold' ? t('ui.foldLines', { n: Number(l.text) }) : l.text}
                    </div>
                  ))}
                </div>
              </div>
            )
          })}
          </>
        ) : (
          <>
            {configError ? (
              <div className="note warn">{t('ui.configFailed', { message: configError })}</div>
            ) : files === null ? (
              <div className="empty">{cwd.trim() ? t('ui.configLoading') : empty}</div>
            ) : files.length === 0 ? (
              <div className="empty">{empty}</div>
            ) : (
              files.map((f) => (
                <div key={f.path}>
                  <div className="group-title">
                    <span className="mono" style={{ textTransform: 'none', letterSpacing: 0 }}>
                      {f.path}
                    </span>
                  </div>
                  <pre className="json">{f.content}</pre>
                </div>
              ))
            )}
          </>
        )}
      </div>

      {/* Same shape as the profile pane's foot: the action filled and on the left,
          the lesser one beside it. One primary per screen, always in one place.
          Both buttons write the draft, so the reading that shows the files has
          neither: a write button under a diff nobody is looking at is a trap. */}
      {view === 'draft' && (
        <div className="pane-foot row-gap">
          <button
            className="btn primary"
            onClick={onApply}
            disabled={
              busy ||
              stale ||
              !resolution ||
              !cwd.trim() ||
              !resolution.changes.some((c) => c.before !== c.after)
            }
          >
            {t('ui.applyFiles')}
          </button>
          <button className="btn" onClick={onResolve} disabled={busy}>
            {t('ui.build')}
          </button>
        </div>
      )}
    </section>
  )
}
