import { useEffect, useState, type ReactNode } from 'react'
import type {
  Card as CardData,
  GeneratedDescription,
  RuleOpinion,
  RuleState,
  SkillState,
} from '../../shared/types.js'
import * as api from '../api.js'
import { formatCost, formatTokens } from '../diff.js'
import { useT } from '../i18n.js'
import { Readme } from '../md.js'
import { toolRule } from '../profile-ops.js'

/**
 * The detail card: what a skill tells the model, what a plugin drags in, which
 * plugin a bare MCP server came from. Read-only on its own — it gains the two
 * controls a profile can actually enforce (a skill's override state, a single
 * MCP tool's permission) only when the pane passes in the draft to write to.
 */
interface Props {
  kind: CardData['kind']
  name: string
  cwd: string
  file?: string
  scope?: string
  origin?: string
  /**
   * A registry row: not installed, so nothing about it is on disk. These are what
   * the row already knew, handed to the card so it can show the version and the
   * transport next to the one thing it fetches — the readme.
   */
  repo?: string
  description?: string
  version?: string
  transport?: string
  installed?: boolean
  installable?: boolean
  busy: boolean
  onInstall?: (id: string) => void
  onClose: () => void

  /** Skill kind: the profile's override for this skill. Absent — the card only reads. */
  skillState?: SkillState
  onSkillState?: (s: SkillState) => void
  /** MCP kind: the profile's permission rules, keyed by rule. */
  ruleStates?: Record<string, RuleOpinion>
  /**
   * MCP kind: every rule the settings files already carry. Read because these
   * boxes are the only place a server's tools are denied — the inventory's rows
   * hold catalog rules, and the catalog has no per-tool MCP rules. Without it the
   * box read the empty profile and stayed ticked over a file that denies the tool.
   */
  fileRuleStates?: Record<string, RuleState>
  onRule?: (rules: string[], state: RuleState) => void
  /** MCP kind: whether the profile keeps the server at all. */
  mcpOn?: boolean
  onMcp?: () => void
}

/** The four states `skillOverrides` accepts, in the order a person would want them. */
const SKILL_STATES: Array<{ v: SkillState; label: string; title: string }> = [
  { v: 'on', label: 'cat.stOn', title: 'cat.stOnTitle' },
  { v: 'name-only', label: 'cat.stNameOnly', title: 'cat.stNameOnlyTitle' },
  { v: 'user-invocable-only', label: 'cat.stUserOnly', title: 'cat.stUserOnlyTitle' },
  { v: 'off', label: 'cat.stOff', title: 'cat.stOffTitle' },
]

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="card-field">
      <span className="card-label">{label}</span>
      <span className="card-value">{children}</span>
    </div>
  )
}

export function Card({
  kind,
  name,
  cwd,
  file,
  scope,
  origin,
  repo,
  description: rowDescription,
  version,
  transport,
  installed,
  installable,
  busy,
  onInstall,
  onClose,
  skillState,
  onSkillState,
  ruleStates,
  fileRuleStates,
  onRule,
  mcpOn,
  onMcp,
}: Props) {
  const t = useT()
  const [data, setData] = useState<CardData | null>(null)
  const [error, setError] = useState<string | undefined>(undefined)
  const [gen, setGen] = useState<GeneratedDescription | null>(null)
  const [busyDesc, setBusyDesc] = useState(false)

  // One query, built the same way for both requests below: the card and the
  // description it asks for are questions about the same entry.
  const query = {
    kind,
    name,
    cwd,
    ...(file ? { file } : {}),
    ...(scope ? { scope } : {}),
    ...(origin ? { origin } : {}),
    ...(repo ? { repo } : {}),
    ...(rowDescription ? { description: rowDescription } : {}),
    ...(version ? { version } : {}),
    ...(transport ? { transport } : {}),
  }

  useEffect(() => {
    let dead = false
    setData(null)
    setError(undefined)
    setGen(null)
    void api
      .getCard(query)
      .then((c) => !dead && setData(c))
      .catch((e: Error) => !dead && setError(e.message))
    return () => {
      dead = true
    }
  }, [kind, name, cwd, file, scope, origin, repo, rowDescription, version, transport])

  /**
   * No description from the author — ask for one we write ourselves. Its own
   * request, because it can reach the network and the model, and it must not
   * hold the card back. The author's own text is never replaced.
   *
   * A registry row already has a readme on screen, and a readme explains the
   * thing better than a sentence we would invent to sit above it — so that card
   * is left alone and the model is never paid for it.
   */
  useEffect(() => {
    if (!data || data.description || (repo && data.body)) return
    let dead = false
    setBusyDesc(true)
    void api
      .describeCard(query)
      .then((g) => {
        if (dead) return
        setGen(g)
        setBusyDesc(false)
      })
      .catch((e: Error) => {
        if (dead) return
        setGen({ error: e.message })
        setBusyDesc(false)
      })
    return () => {
      dead = true
    }
  }, [data, kind, name, cwd, file, scope, origin, repo, rowDescription, version, transport])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  // The author's own words when there are any, ours when there are none. Never a
  // translation: a description is the text its author wrote.
  const description = data?.description ? data.description : gen?.text
  // Explicit boolean: a falsy `0` from the lengths would otherwise print itself.
  const hasBundled = Boolean(
    data && (data.bundledSkills.length || data.bundledMcp.length || data.owners.length),
  )

  return (
    <aside className="card">
      <div className="card-head">
        <span className="mono">{name}</span>
        <span style={{ flex: 1 }} />
        <button className="btn icon" title={t('cat.close')} onClick={onClose}>
          ✕
        </button>
      </div>

      <div className="card-body">
        {error && <div className="note warn">{error}</div>}

        {description ? (
          <>
            <div className="card-label">{t('cat.cardTitle')}</div>
            <p className="card-desc">{description}</p>
          </>
        ) : busyDesc ? (
          <div className="muted">{t('cat.describing')}</div>
        ) : gen?.error ? (
          <div className="note warn">{t('cat.descFailed', { error: gen.error })}</div>
        ) : (
          data && <div className="note">{t('cat.noDescription')}</div>
        )}

        {data?.body ? (
          <>
            <div className="card-label">{repo ? t('cat.readme') : t('cat.body')}</div>
            <Readme
              text={data.body}
              // A readme is full of `docs/setup.md`. Its own repository is what
              // those are relative to, and that is a link we already hold.
              {...(data.link?.startsWith('https://github.com/')
                ? { base: `${data.link}/blob/HEAD/` }
                : {})}
            />
          </>
        ) : data && kind === 'skill' ? (
          <div className="muted">{t('cat.bodyEmpty')}</div>
        ) : null}

        {hasBundled && data && (
          <>
            <div className="card-label">{t('cat.bundled')}</div>
            <div className="card-chips">
              {data.owners.map((o) => (
                <span className="chip origin" key={o} title={t('cat.owners')}>
                  {o}
                </span>
              ))}
              {data.bundledSkills.map((s) => (
                <span className="chip" key={s}>
                  {s}
                </span>
              ))}
              {data.bundledMcp.map((m) => (
                <span className="chip measured" key={m}>
                  {m}
                </span>
              ))}
            </div>
          </>
        )}

        {data && (
          <div className="stack-gap" style={{ marginTop: 10 }}>
            {data.link && (
              <Field label={t('cat.link')}>
                <a href={data.link} target="_blank" rel="noreferrer">
                  {data.link.replace(/^https:\/\/github\.com\//, '')}
                </a>
              </Field>
            )}
            {data.file && <Field label={t('cat.file')}>{data.file}</Field>}
            {data.scope && <Field label={t('cat.scope')}>{data.scope}</Field>}
            {data.origin && <Field label={t('cat.origin')}>{data.origin}</Field>}
            {data.marketplace && <Field label={t('cat.marketplace')}>{data.marketplace}</Field>}
            {data.category && <Field label={t('cat.category')}>{data.category}</Field>}
            {data.version && <Field label={t('cat.version')}>{data.version}</Field>}
            {data.transport && <Field label={t('cat.transport')}>{data.transport}</Field>}
            {data.installs !== undefined && (
              <Field label={t('cat.installs')}>{t('inv.installs', { n: data.installs })}</Field>
            )}
            {data.modelInvocable !== undefined && (
              <Field label={t('cat.modelInvocable')}>{data.modelInvocable ? '✓' : '—'}</Field>
            )}
            {data.cost && <Field label={t('cat.cost')}>{formatCost(data.cost)}</Field>}
          </div>
        )}

        {data && kind === 'skill' && onSkillState && (
          <>
            <div className="card-label">{t('cat.skillState')}</div>
            <div className="seg">
              {SKILL_STATES.map((s) => (
                <button
                  key={s.v}
                  data-on={s.v === skillState}
                  title={t(s.title)}
                  onClick={() => onSkillState(s.v)}
                >
                  {t(s.label)}
                </button>
              ))}
            </div>
            <div className="note tiny">{t('cat.skillStateHint')}</div>
          </>
        )}

        {data && kind === 'mcp' && onMcp && (
          <>
            <div className="card-label">{t('cat.server')}</div>
            {/* One box for the whole server, above the thirty the server hands over:
                the tools are a detail, and the thing a person usually wants off is the
                server — its instructions and its protocol envelope go with it. */}
            <label className="tool-row">
              <span className="tool-head">
                <input type="checkbox" checked={mcpOn !== false} onChange={() => onMcp()} />
                <span className="mono">{name}</span>
              </span>
            </label>
            <div className="note tiny">{t('cat.serverHint')}</div>
          </>
        )}

        {data && kind === 'mcp' && onRule && (
          <>
            <div className="card-label">{t('cat.tools')}</div>
            {data.tools?.length ? (
              <>
                <div className="stack-gap">
                  {data.tools.map((tool) => {
                    const rule = toolRule(name, tool.name)
                    // The box reads as "this tool is on" — a server hands over all
                    // its tools, so no rule anywhere means checked. Unticking denies
                    // it; ticking a denied tool deletes the rule, which is exactly
                    // what «Разрешить» does in the inventory, and for the same
                    // reason: an `allow` loses to a deny sitting in any other file.
                    //
                    // A rule the profile says nothing about falls to the file's own
                    // `permissions.deny` — this box is the only place those are read,
                    // and reading the profile alone kept them ticked over a file that
                    // denies them.
                    const own = ruleStates?.[rule]
                    const denied = (own ?? fileRuleStates?.[rule]) === 'deny'
                    return (
                      <label key={tool.name} className="tool-row">
                        <span className="tool-head">
                          <input
                            type="checkbox"
                            checked={!denied}
                            onChange={(ev) => onRule([rule], ev.target.checked ? 'allow' : 'deny')}
                          />
                          <span className="mono">{tool.name}</span>
                          <span className="chip measured" title={t('cat.toolCostTitle')}>
                            ≈{formatTokens(tool.tokens)}
                          </span>
                        </span>
                        {tool.desc && (
                          <span className="desc tiny">{tool.desc}</span>
                        )}
                      </label>
                    )
                  })}
                </div>
                <div className="note tiny">{t('cat.toolsHint')}</div>
              </>
            ) : (
              <div className="muted">{t('cat.toolsNoProbe')}</div>
            )}
          </>
        )}

        {data?.notes.map((n, i) => (
          <div className="note" key={i}>
            {n}
          </div>
        ))}
      </div>

      {installable && (
        <div className="card-foot">
          <button className="btn primary" disabled={busy || installed} onClick={() => onInstall?.(name)}>
            {installed ? t('cat.installed') : t('inv.install')}
          </button>
        </div>
      )}
    </aside>
  )
}
