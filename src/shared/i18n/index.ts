/**
 * One dictionary for both the UI and server messages.
 *
 * Browser-safe on purpose: no `node:*` imports here, because the UI bundle pulls
 * this file in. The request-scoped language lives in `src/server/lang.ts`, the
 * React context in `src/ui/i18n.tsx` — both are thin adapters over `t()`.
 *
 * A message is a template with `{name}` placeholders. Keys are flat and prefixed
 * by area (`inv.*`, `sess.*`, `net.*`), so one area module owns its slice of the
 * dictionary without colliding with its neighbours.
 */
import { cat } from './messages/cat.js'
import { core } from './messages/core.js'
import { inv } from './messages/inv.js'
import { mod } from './messages/mod.js'
import { net } from './messages/net.js'
import { prof } from './messages/prof.js'
import { scan } from './messages/scan.js'
import { sess } from './messages/sess.js'
import { stats } from './messages/stats.js'
import { ui } from './messages/ui.js'
import { usage } from './messages/usage.js'

export type Lang = 'ru' | 'en'
export type Params = Record<string, string | number>

/** A module's contribution: the same key set in every language. */
export interface I18nModule {
  ru: Record<string, string>
  en: Record<string, string>
}

export const LANGS: readonly Lang[] = ['ru', 'en']

const MODULES: I18nModule[] = [core, ui, inv, cat, prof, sess, scan, net, stats, usage, mod]

function merge(lang: Lang): Record<string, string> {
  const out: Record<string, string> = {}
  for (const m of MODULES) Object.assign(out, m[lang])
  return out
}

const DICT: Record<Lang, Record<string, string>> = { ru: merge('ru'), en: merge('en') }

export function isLang(v: unknown): v is Lang {
  return v === 'ru' || v === 'en'
}

/**
 * A missing key renders as the key itself: a visible `inv.foo` in the UI is a
 * bug report, an empty string is a mystery. Falls back to Russian before that.
 */
export function t(lang: Lang, key: string, params?: Params): string {
  const s = DICT[lang][key] ?? DICT.ru[key] ?? key
  if (!params) return s
  return s.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  )
}

/** Keys of one language — for the dictionary-parity test. */
export function keysOf(lang: Lang): string[] {
  return Object.keys(DICT[lang])
}
