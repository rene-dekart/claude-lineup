export type DiffLine = { type: 'same' | 'add' | 'del' | 'fold'; text: string }

/**
 * Line-by-line diff via LCS. ponytail: O(n*m) over lines — fine for settings
 * files (hundreds of lines), no reason to rewrite it on Myers.
 */
export function diffLines(a: string, b: string): DiffLine[] {
  const A = a.split('\n')
  const B = b.split('\n')
  const n = A.length
  const m = B.length
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0))

  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i]![j] = A[i] === B[j] ? dp[i + 1]![j + 1]! + 1 : Math.max(dp[i + 1]![j]!, dp[i]![j + 1]!)
    }
  }

  const out: DiffLine[] = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      out.push({ type: 'same', text: A[i]! })
      i++
      j++
    } else if (dp[i + 1]![j]! >= dp[i]![j + 1]!) {
      out.push({ type: 'del', text: A[i]! })
      i++
    } else {
      out.push({ type: 'add', text: B[j]! })
      j++
    }
  }
  while (i < n) out.push({ type: 'del', text: A[i++]! })
  while (j < m) out.push({ type: 'add', text: B[j++]! })
  return out
}

/**
 * The same diff, minus the unchanged bulk.
 *
 * `~/.claude.json` is thousands of lines of other projects' state that this
 * write does not touch. Diffing all of it is both a wall of muted text and an
 * O(n*m) LCS in the browser over a few million cells. The common head and tail
 * are trimmed first — where the edit is not — and only what is left is diffed.
 * Nothing is hidden: each elided run is folded into one line that counts itself.
 */
export function diffFolded(a: string, b: string, context = 3): DiffLine[] {
  // A settings file is short enough to read as itself, and that is the point of
  // showing it whole. Only the big shared file is folded.
  if (a.length < 20_000 && b.length < 20_000) return diffLines(a, b)
  const A = a.split('\n')
  const B = b.split('\n')
  let head = 0
  while (head < A.length && head < B.length && A[head] === B[head]) head++
  let tail = 0
  while (
    tail < A.length - head &&
    tail < B.length - head &&
    A[A.length - 1 - tail] === B[B.length - 1 - tail]
  ) {
    tail++
  }

  const out: DiffLine[] = []
  const keptHead = Math.min(head, context)
  for (let i = 0; i < keptHead; i++) out.push({ type: 'same', text: A[i]! })
  if (head > keptHead) out.push({ type: 'fold', text: String(head - keptHead) })
  out.push(
    ...diffLines(A.slice(head, A.length - tail).join('\n'), B.slice(head, B.length - tail).join('\n')),
  )
  if (tail > context) out.push({ type: 'fold', text: String(tail - context) })
  for (let i = Math.max(0, A.length - Math.min(tail, context)); i < A.length; i++) {
    out.push({ type: 'same', text: A[i]! })
  }
  return out
}

/** A bare count: for numbers whose provenance the surrounding text already states. */
export function formatTokens(t: number): string {
  return t >= 1000 ? `${(t / 1000).toFixed(1)}k` : String(t)
}

export function formatCost(c: { level: string; tokens?: number; source?: string }): string {
  if (c.tokens === undefined) return '—'
  const s = formatTokens(c.tokens)
  // Three kinds of number, three marks. A session count is what Claude Code
  // really sent, so it is shown bare; ≈ is our own measurement through a
  // proxy (chars/4); ~ is a file size over 4. Mixing them silently is how a
  // guess gets read as a fact.
  if (c.source === 'session') return s
  return c.level === 'measured' ? `≈${s}` : `~${s}`
}

/** Which explanation belongs to a figure — the chip's tooltip. */
export function sourceKey(c: { tokens?: number; source?: string }): string {
  if (c.tokens === undefined) return 'prof.srcNone'
  if (c.source === 'session') return 'prof.srcSession'
  if (c.source === 'probe') return 'prof.srcProbe'
  return 'prof.srcSize'
}
