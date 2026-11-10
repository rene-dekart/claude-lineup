import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { LANGS, isLang, t as translate, type Lang, type Params } from '../shared/i18n/index.js'
import { setApiLang } from './api.js'

const STORAGE_KEY = 'cc-lang'

/**
 * The modifier key as this platform writes it. The hotkey was always Ctrl here —
 * only the labels said ⌘, and this is a Windows page (see the design spec's
 * cross-platform note). Shared so the topbar button and the palette footer
 * cannot disagree about it.
 */
export const MOD = /Mac|iPhone|iPad/.test(navigator.userAgent) ? '⌘' : 'Ctrl'

function loadLang(): Lang {
  try {
    const v = localStorage.getItem(STORAGE_KEY)
    return isLang(v) ? v : 'ru'
  } catch {
    return 'ru' // storage disabled (private mode) — start from the default
  }
}

/** Keeps the API in sync: every request carries `x-lang` from here on. */
function saveLang(lang: Lang): void {
  try {
    localStorage.setItem(STORAGE_KEY, lang)
  } catch {
    /* storage disabled — the choice lives for this page view only */
  }
  setApiLang(lang)
}

interface LangState {
  lang: Lang
  setLang: (l: Lang) => void
}

const LangContext = createContext<LangState | null>(null)

/**
 * Owns the language for the whole tree and sits ABOVE App on purpose: a
 * component cannot read a context it renders itself, so a provider inside App
 * would leave App's own strings (status bar, buttons) stuck on the default.
 */
export function LangRoot({ children }: { children: ReactNode }) {
  const [lang, setLang] = useState(loadLang)

  useEffect(() => {
    saveLang(lang)
    document.documentElement.lang = lang
  }, [lang])

  return <LangContext.Provider value={{ lang, setLang }}>{children}</LangContext.Provider>
}

function useLangState(): LangState {
  const v = useContext(LangContext)
  if (!v) throw new Error('LangRoot is missing above this component')
  return v
}

export function useT(): (key: string, params?: Params) => string {
  const { lang } = useLangState()
  return (key, params) => translate(lang, key, params)
}

/**
 * The current language, as state. For anything that FORMATS instead of
 * translating: numbers, dates. `useT` hides it, and reading
 * `document.documentElement.lang` instead would pin the format to whatever the
 * language was when the panel mounted.
 */
export function useLang(): Lang {
  return useLangState().lang
}

/** Shows the language you would switch TO. */
export function LangToggle() {
  const { lang, setLang } = useLangState()
  const next = LANGS[(LANGS.indexOf(lang) + 1) % LANGS.length] ?? 'ru'
  return (
    <button
      className="btn icon"
      title={lang === 'ru' ? 'Switch the interface to English' : 'Язык интерфейса: русский'}
      onClick={() => setLang(next)}
    >
      {next.toUpperCase()}
    </button>
  )
}
