import type { ReactNode } from 'react'

/**
 * A README, rendered — not a markdown engine.
 *
 * The text is somebody else's, so none of it ever becomes HTML: every block is a
 * React element and React escapes what it is handed. What the renderer does have
 * to do is drop the parts of a GitHub README that are markup rather than prose —
 * the badge strip, the centred `<div>` header, the mermaid diagram. A card that
 * prints those raw is a card nobody reads.
 *
 * ponytail: headings, lists, tables, fences and inline bold/code/links only.
 * Add when: a README shows something this drops and it matters.
 */

export type Block =
  | { t: 'h'; level: number; text: string }
  | { t: 'p'; text: string }
  | { t: 'ul'; items: string[] }
  | { t: 'code'; text: string }
  | { t: 'table'; head: string[]; rows: string[][] }

const FENCE = /^\s*(?:```|~~~)(\w*)/
const HEAD = /^(#{1,6})\s+(.*)$/
const ITEM = /^\s*(?:[-*+]|\d+\.)\s+(.*)$/
const RULE = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/
const ROW = /^\s*\|(.+)\|\s*$/
const SEP = /^\s*\|[\s:|-]+\|\s*$/
/**
 * A line of markup — `<div align="center">`, `<a href=…>`, `</p>`. An HTML block
 * in a README is one such line after another, so a single test covers the lot.
 */
const HTML = /^\s*</
/** A line that is only a picture: the shields.io badges, mostly. */
const PICTURE = /^\s*\[?!\[/

const cells = (line: string): string[] =>
  line
    .trim()
    .replace(/^\||\|$/g, '')
    .split('|')
    .map((c) => c.trim())

/** Blocks of a README, in order. */
export function blocks(md: string): Block[] {
  const out: Block[] = []
  const lines = md.split(/\r?\n/)
  let para: string[] = []

  const flush = () => {
    const text = para.join(' ').trim()
    if (text) out.push({ t: 'p', text })
    para = []
  }

  for (let i = 0; i < lines.length; i++) {
    const line = (lines[i] ?? '').trimEnd()

    const fence = FENCE.exec(line)
    if (fence) {
      flush()
      const lang = fence[1] ?? ''
      const body: string[] = []
      while (++i < lines.length && !FENCE.test(lines[i] ?? '')) body.push(lines[i] ?? '')
      // A diagram is not text: its source is a wall of arrows and node names,
      // and drawing it is not this renderer's job.
      if (lang !== 'mermaid') out.push({ t: 'code', text: body.join('\n') })
      continue
    }

    if (!line.trim() || HTML.test(line) || PICTURE.test(line) || RULE.test(line)) {
      flush()
      continue
    }

    const head = HEAD.exec(line)
    if (head) {
      flush()
      out.push({ t: 'h', level: (head[1] ?? '#').length, text: (head[2] ?? '').trim() })
      continue
    }

    // Only a table if the next line is its separator: a lone `|` line is prose.
    if (ROW.test(line) && SEP.test(lines[i + 1] ?? '')) {
      flush()
      const head = cells(line)
      const rows: string[][] = []
      // Past the header and the `|---|` under it: that line is the table's
      // format, not one of its rows.
      let j = i + 2
      while (j < lines.length && ROW.test(lines[j] ?? '') && !SEP.test(lines[j] ?? '')) {
        rows.push(cells(lines[j] ?? ''))
        j++
      }
      i = j - 1
      out.push({ t: 'table', head, rows })
      continue
    }

    const item = ITEM.exec(line)
    if (item) {
      flush()
      const items: string[] = []
      for (let m: RegExpExecArray | null = item; m; m = ITEM.exec(lines[i] ?? '')) {
        items.push((m[1] ?? '').trim())
        i++
      }
      i--
      out.push({ t: 'ul', items })
      continue
    }

    para.push(line.trim())
  }

  flush()
  return out
}

/**
 * Wiki links first: `[[concepts/agent-memory]]` is markdown only by accident, and a
 * parser that met it as `[label](url)` would need a `](` the wiki never writes.
 */
const INLINE =
  /\[\[([^\]|#]*)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]|!?\[([^\]]*)\]\(([^)\s]+)[^)]*\)|\*\*([^*]+)\*\*|__([^_]+)__|`([^`]+)`/g

/** How a wiki link is drawn: where its target lands, and how to open one. */
export interface WikiLinks {
  /** Target as written in the page → a path inside the KB, or null for a dead one. */
  links: Record<string, string | null>
  open: (path: string) => void
}

/**
 * http(s), mailto and in-page targets pass. This is a stranger's text and the
 * href is the one place it gets to name a destination, so `javascript:` never
 * reaches the DOM.
 *
 * A relative target — `docs/setup.md` — is a link only when we know what it is
 * relative to, which is the repository the readme came from: without that base
 * it names nothing, and printing it as a link would send the window somewhere
 * else entirely.
 */
const safeHref = (url: string, base?: string): string | undefined => {
  const u = url.trim()
  if (/^(https?:|mailto:)/i.test(u)) return u
  if (u.startsWith('#')) return u
  return base && !/^[a-z][\w+.-]*:/i.test(u) ? base + u.replace(/^\.?\//, '') : undefined
}

export function inline(text: string, key = '', base?: string, wiki?: WikiLinks): ReactNode[] {
  const out: ReactNode[] = []
  let last = 0
  let n = 0
  for (const m of text.matchAll(INLINE)) {
    const at = m.index ?? 0
    if (at > last) out.push(text.slice(last, at))
    last = at + m[0].length
    const k = `${key}${n++}`
    // An image: a badge, or a diagram this cannot draw. Its alt text is a label
    // for a picture nobody is seeing.
    if (m[0].startsWith('!')) continue
    if (m[1] !== undefined) {
      // No knowledge base behind this text — a plugin's README, say — and the
      // brackets are somebody's prose. They stay as written.
      if (!wiki || !m[1].trim()) {
        out.push(m[0])
        continue
      }
      const target = m[1].trim()
      // A target nothing answers to is left as text, not as a button that opens
      // nothing: the wiki writes such links on purpose (1639 of them here).
      const path = Object.hasOwn(wiki.links, target) ? wiki.links[target] : null
      const label = (m[2] ?? '').trim() || (target.split('/').pop() ?? target)
      out.push(
        path ? (
          <button key={k} className="wikilink" onClick={() => wiki.open(path)}>
            {label}
          </button>
        ) : (
          <span key={k} className="wikilink dead" title={target}>
            {label}
          </span>
        ),
      )
      continue
    }
    if (m[3] !== undefined && m[4] !== undefined) {
      const href = safeHref(m[4], base)
      // The label is inline text of its own — `[**Live API docs**](…)` is how
      // half of GitHub writes a link, and the asterisks are not the label.
      out.push(
        href ? (
          <a key={k} href={href} target="_blank" rel="noreferrer">
            {inline(m[3], `${k}a`, base, wiki)}
          </a>
        ) : (
          inline(m[3], `${k}a`, base, wiki)
        ),
      )
      continue
    }
    if (m[5] !== undefined) out.push(<b key={k}>{m[5]}</b>)
    else if (m[6] !== undefined) out.push(<b key={k}>{m[6]}</b>)
    else if (m[7] !== undefined) out.push(<code key={k}>{m[7]}</code>)
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}

function view(b: Block, i: number, base?: string, wiki?: WikiLinks): ReactNode {
  const k = `${i}.`
  switch (b.t) {
    case 'h': {
      // A README's `#` is the card's section title, not the page's: the card is
      // already a section of the window, so its headings start two levels down.
      const Tag = `h${Math.min(b.level + 2, 6)}` as 'h3' | 'h4' | 'h5' | 'h6'
      return <Tag key={i}>{inline(b.text, k, base, wiki)}</Tag>
    }
    case 'ul':
      return (
        <ul key={i}>
          {b.items.map((it, j) => (
            <li key={j}>{inline(it, `${k}${j}.`, base, wiki)}</li>
          ))}
        </ul>
      )
    case 'code':
      return (
        <pre className="md-code" key={i}>
          {b.text}
        </pre>
      )
    case 'table':
      return (
        <div className="md-table" key={i}>
          <table>
            <thead>
              <tr>
                {b.head.map((c, j) => (
                  <th key={j}>{inline(c, `${k}h${j}.`, base, wiki)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {b.rows.map((r, j) => (
                <tr key={j}>
                  {r.map((c, l) => (
                    <td key={l}>{inline(c, `${k}${j}.${l}.`, base, wiki)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )
    default:
      return <p key={i}>{inline(b.text, k, base, wiki)}</p>
  }
}

/**
 * `base` is what a relative link in the readme is relative to — its repository.
 * `wiki` is the knowledge base a page's `[[links]]` belong to; without it they stay text.
 */
export function Readme({ text, base, wiki }: { text: string; base?: string; wiki?: WikiLinks }) {
  const all = blocks(text)
  return <div className="md">{all.map((b, i) => view(b, i, base, wiki))}</div>
}
