import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import type {
  Entity,
  Inventory,
  ModuleRow,
  Profile,
  Resolution,
  RuleState,
  SkillState,
} from '../shared/types.js'
import * as api from './api.js'
import { BackupsOverlay } from './components/BackupsOverlay.js'
import { Card } from './components/Card.js'
import { CwdField } from './components/CwdField.js'
import { CatalogOverlay } from './components/CatalogOverlay.js'
import { CommandPalette, type PaletteItem } from './components/CommandPalette.js'
import { ContextMeter } from './components/ContextMeter.js'
import {
  IconBars,
  IconBolt,
  IconClock,
  IconGrid,
  IconMoon,
  IconPuzzle,
  IconPlug,
  IconSliders,
  IconSun,
} from './components/Icon.js'
import { InventoryPane } from './components/InventoryPane.js'
import { ProfilePane } from './components/ProfilePane.js'
import { ResultPane } from './components/ResultPane.js'
import { StatsPage } from './components/StatsPage.js'
import { UsagePage } from './components/UsagePage.js'
import { ModulesPage } from './modules/ModulesPage.js'
import { ModuleView, moduleIcon } from './modules/registry.js'
import { LangToggle, MOD, useT } from './i18n.js'
import {
  deadSkillStates,
  isMcpOn,
  isPluginOn,
  isSkillOn,
  setRules,
  setSkillState,
  toggleMcp,
  togglePlugin,
  toggleSkill,
} from './profile-ops.js'

// Fresh profile: every source on, so the session starts as the machine already is
// and the profile is read as a list of things to *turn off*. Dropping `user` here
// would silently take the globals with it — 0 of 50 skills on first open, which
// looks like a broken app rather than a choice.
/** Narrowest width each of the two draggable panes may be squeezed to. */
const MIN: [number, number] = [260, 300]
/** What the third pane keeps, and the two 4px handles between the three. */
const RESULT_MIN = 320
const HANDLES = 8

/** The rail: an icon strip at its narrowest, a titled list at its widest. */
const RAIL_MIN = 48
const RAIL_MAX = 240
/** Past this the row has room for its name — below it the name would be cut. */
const RAIL_LABEL = 132

const clampRail = (w: number) => Math.min(Math.max(w, RAIL_MIN), RAIL_MAX)

/**
 * The places the rail switches between. Whichever is chosen replaces the workspace
 * wholesale — they are not windows over it, and there is only ever one of them, which
 * is what the lit rail row says.
 *
 * `mod:<id>` is a module's own view. A template literal, not a fixed member: the
 * core does not know which modules exist, so it cannot name their destinations.
 */
export type View =
  | 'settings'
  | 'catalogPlugins'
  | 'catalogMcp'
  | 'stats'
  | 'usage'
  | 'history'
  | 'modules'
  | `mod:${string}`

/** The destinations the core names. A module's own is `mod:<id>`, matched below. */
const FIXED_VIEWS: readonly string[] = [
  'settings',
  'catalogPlugins',
  'catalogMcp',
  'stats',
  'usage',
  'history',
  'modules',
]

/**
 * The destination to restore, or the default.
 *
 * A string from an older build is not a destination: the rail would light no row
 * and the page would be blank. The fixed names are checked here, and `mod:` is
 * taken as written — the core does not know which modules exist, and one that is
 * gone says so itself rather than leaving the user on an empty screen.
 */
function storedView(raw: string | null): View {
  if (raw === null) return 'settings'
  if (raw.startsWith('mod:')) return raw as View
  return FIXED_VIEWS.includes(raw) ? (raw as View) : 'settings'
}

const EMPTY: Profile = {
  id: 'new',
  title: '',
  permissions: { rules: {} },
  mcp: { mode: 'all', servers: [] },
  plugins: { enabled: [], disabled: [] },
  skills: { mode: 'all', states: {} },
  hooks: { disableAll: false },
  memory: { autoDiscover: true },
  flags: [],
  claude: {},
}

export function App() {
  const t = useT()
  const [theme, setTheme] = useState<'light' | 'dark'>(
    () => (localStorage.getItem('cc-theme') as 'light' | 'dark') ?? 'dark',
  )
  const [cwd, setCwd] = useState(() => localStorage.getItem('cc-cwd') ?? '')
  const [inventory, setInventory] = useState<Inventory | null>(null)
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [draft, setDraft] = useState<Profile>(() => ({ ...EMPTY, title: t('ui.newProfile') }))
  const [result, setResult] = useState<{ resolution: Resolution } | null>(null)
  const [status, setStatus] = useState('')
  const [busy, setBusy] = useState(false)
  /** `null` until the list arrives — not the same as "this build carries none". */
  const [mods, setMods] = useState<ModuleRow[] | null>(null)

  /**
   * The module list is the rail's second half: a module that is on adds a row of its
   * own. Fetched once at mount; the catalog page keeps it current afterwards.
   *
   * Unprobed, so this costs the app nothing: no module's tool is looked for until a
   * human opens its catalog row. A module the machine has never seen is still a row
   * in the rail once it is switched on — switching it on was a human's decision, and
   * the rail's job is to remember it.
   */
  useEffect(() => {
    void api
      .getModules()
      // Whoever answers first wins, and this is the cheap answer: the catalog page
      // asks for the same list WITH its tool checks, and its answer is the better one.
      // Without this, the two race and the plain list can overwrite a probed one —
      // the catalog then shows "not checked" beside a tool that has answered.
      .then((r) => setMods((prev) => prev ?? r.modules))
      // A catalog that cannot be read hides nothing that matters: the modules are
      // optional, so silence costs a tab and never the app.
      .catch(() => setMods((prev) => prev ?? []))
  }, [])

  const [paletteOpen, setPaletteOpen] = useState(false)
  /**
   * Pane widths in px, the user's to keep. The third pane takes what is left,
   * so only the first two are stored.
   */
  const [cols, setCols] = useState<[number, number]>(() => {
    try {
      const v: unknown = JSON.parse(localStorage.getItem('cc-cols') ?? '')
      if (Array.isArray(v) && typeof v[0] === 'number' && typeof v[1] === 'number') {
        return [v[0] as number, v[1] as number]
      }
    } catch {
      /* nothing saved yet, or it was garbage */
    }
    return [420, 480]
  })
  /** The rail's width, kept the same way and for the same reason as the panes'. */
  const [railW, setRailW] = useState(() => {
    const v = Number(localStorage.getItem('cc-rail'))
    return clampRail(Number.isFinite(v) && v > 0 ? v : RAIL_MIN)
  })
  const colsRef = useRef<HTMLDivElement>(null)
  const drag = useRef<{ i: 0 | 1; startX: number; from: [number, number] } | null>(null)
  const railDrag = useRef<{ startX: number; from: number } | null>(null)
  /**
   * Where the user is standing. One destination at a time; the rail is the way
   * from any of them to any other. Statistics walks every transcript, so it asks
   * for them on entry — never at app load.
   */
  const [view, setView] = useState<View>(() => storedView(localStorage.getItem('cc-view')))

  /**
   * A module can be switched off while its own destination is the one the browser
   * remembers — F5 after turning it off in the catalog. The rail then has no such row
   * and the remembered view would draw a page whose server answers 409. So the view
   * steps back to Settings, the one destination that always exists. Nothing is decided
   * until the list has arrived: `null` is "not asked yet", not "gone".
   */
  useEffect(() => {
    if (!view.startsWith('mod:') || mods === null) return
    if (mods.some((m) => m.enabled && m.id === view.slice(4))) return
    setView('settings')
  }, [view, mods])
  /**
   * The restore point of the last apply — the statusbar's undo button, which
   * only makes sense while the write it belongs to is still the last one. Any
   * recompute (rescan, resolve, another apply) retires it.
   */
  const [undoPoint, setUndoPoint] = useState<string | null>(null)
  /** A card opened from the inventory — the catalog window renders its own. */
  const [openCard, setOpenCard] = useState<{ kind: Entity['kind']; name: string; file?: string } | null>(
    null,
  )
  /**
   * What "all" covers: the profile stores MCP as a mode, not a list, so every
   * toggle — a row, the palette, the card — needs the inventory's own names. The
   * card is only ever opened from the inventory, so the list is there when it is.
   */
  const allMcpNames = [
    ...new Set((inventory?.entities ?? []).filter((e) => e.kind === 'mcp').map((e) => e.name)),
  ]
  /**
   * The names the files refuse to load, which is what the boxes untick under
   * "all". The MCP toggle needs them: it turns the boxes into a list, and a name
   * it cannot see would be ticked by the write rather than by the click.
   */
  const deniedMcpNames = [
    ...new Set(
      (inventory?.entities ?? []).flatMap((e) =>
        e.kind === 'mcp' && e.offByFile ? [e.name] : [],
      ),
    ),
  ]
  /** What the files say about one name — the defaults the toggles start from. */
  const mcpOffByFile = (name: string) => deniedMcpNames.includes(name)
  const skillOffByFile = (name: string) =>
    (inventory?.entities ?? []).some((e) => e.kind === 'skill' && e.name === name && e.offByFile)

  useEffect(() => {
    document.documentElement.dataset['theme'] = theme
    localStorage.setItem('cc-theme', theme)
  }, [theme])

  // Where the user was standing, kept like the theme: a reload used to drop them
  // back on Settings with no way to tell what they had been looking at.
  useEffect(() => {
    localStorage.setItem('cc-view', view)
  }, [view])

  useEffect(() => {
    localStorage.setItem('cc-cwd', cwd)
  }, [cwd])

  // ⌘K / Ctrl+K. Escape is handled by the palette's own input, which keeps
  // focus for as long as it is open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey) && !e.altKey) {
        e.preventDefault()
        setPaletteOpen((v) => !v)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const scan = useCallback(async (target: string) => {
    // The inventory is an input to the resolution — a rescan can change what the
    // profile resolves to, and the last answer is about the machine as it was.
    setResult(null)
    setUndoPoint(null)
    if (!target.trim()) {
      setInventory(null)
      return
    }
    setBusy(true)
    setStatus('')
    try {
      setInventory(await api.getInventory(target))
    } catch (e) {
      setStatus(t('ui.inventoryError', { message: (e as Error).message }))
    } finally {
      setBusy(false)
    }
    // `t` is stable in practice — the provider it reads lives below App — and
    // must stay out of the deps: a new identity each render would re-fire this.
  }, [])

  useEffect(() => {
    void api
      .getProfiles()
      .then(setProfiles)
      .catch((e: Error) => setStatus(t('ui.profilesUnreadable', { message: e.message })))
  }, [])

  useEffect(() => {
    void scan(cwd)
  }, [cwd, scan])

  /**
   * Whether the diff on screen still describes the draft.
   *
   * Every edit retires the build it makes untrue — the diff and Apply's own
   * enabled state both read off a resolution the edit has just invalidated. It
   * used to be thrown away outright, which left the pane with no diff and falling
   * back to the patch preview: the MCP switch is not a line in `settingsPatch`, so
   * a click on a server's box appeared to wipe the file's diff down to the one
   * skill line the profile had changed. Ticking a box on and off made the pane
   * jump between two answers. The edit now only marks the build stale and a
   * debounced rebuild replaces it; the old diff stays readable for that moment,
   * and Apply stays off until the new one lands.
   */
  const [stale, setStale] = useState(false)
  /** Newest build wins — a slower answer must not overwrite a later one. */
  const buildSeq = useRef(0)

  const patch = (p: Partial<Profile>) => {
    setStale(true)
    setDraft((d) => ({ ...d, ...p }))
  }

  /** The same invalidation, for a profile swapped in whole. */
  const loadProfile = (p: Profile) => {
    setStale(true)
    setDraft(p)
  }

  const doResolve = async () => {
    const token = ++buildSeq.current
    setBusy(true)
    setStatus('')
    setUndoPoint(null)
    try {
      const r = await api.resolveProfile(draft, cwd)
      if (token === buildSeq.current) {
        setResult(r)
        setStale(false)
      }
    } catch (e) {
      if (token === buildSeq.current) setStatus(t('ui.resolveFailed', { message: (e as Error).message }))
    } finally {
      setBusy(false)
    }
  }

  /**
   * The draft is what the diff has to describe, so an edit rebuilds it on its
   * own; «Собрать» stays as the way to ask for a build by hand. Debounced,
   * because a person dragging over a column of checkboxes is making one change,
   * not one per click. The first run is skipped: before anything is edited the
   * pane's job is the patch the profile asks for, not a diff against a profile
   * nobody has touched.
   */
  const edited = useRef(false)
  useEffect(() => {
    if (!edited.current) {
      edited.current = true
      return
    }
    if (!cwd.trim()) return
    const token = ++buildSeq.current
    const id = setTimeout(() => {
      api
        .resolveProfile(draft, cwd)
        .then((r) => {
          if (token !== buildSeq.current) return
          setResult(r)
          setStale(false)
        })
        // The rebuild is a preview: if it cannot be had, the old diff simply stays
        // stale rather than the pane refusing a write the server would perform.
        .catch(() => {
          if (token === buildSeq.current) setStale(false)
        })
    }, 250)
    return () => clearTimeout(id)
  }, [draft, cwd])

  /**
   * Entries in the draft's override map that Claude Code ignores, dropped as soon
   * as the inventory can name them.
   *
   * A plugin's skill was offered a box that wrote one — the map does not reach
   * inside a plugin — so a draft can carry such keys, and the build writes them
   * into the user's file where they decide nothing. Removing them is the one edit
   * that is not a person's, and it is the edit that makes the diff and the launch
   * agree again; the rebuild follows on its own, because the draft moved.
   */
  useEffect(() => {
    if (!inventory) return
    const dead = deadSkillStates(draft, inventory.entities)
    if (dead.length === 0) return
    setStale(true)
    setDraft((d) => {
      const states = { ...d.skills.states }
      for (const n of dead) delete states[n]
      return { ...d, skills: { ...d.skills, states } }
    })
    // The map itself is a dependency, not only the inventory: a saved profile is
    // loaded into the draft after the inventory has arrived, and that is where
    // these keys mostly come from — a profile written when a plugin's skill still
    // had a box. It finds nothing on its own rewrite, so it stops there.
  }, [inventory, draft.skills.states])

  const doSaveProfile = async () => {
    setBusy(true)
    try {
      const saved = await api.saveProfile(draft)
      setDraft(saved)
      setProfiles(await api.getProfiles())
      setStatus(t('ui.profileSaved', { title: saved.title }))
    } catch (e) {
      setStatus(t('ui.saveFailed', { message: (e as Error).message }))
    } finally {
      setBusy(false)
    }
  }

  const doApply = async () => {
    const token = ++buildSeq.current
    setBusy(true)
    setUndoPoint(null)
    try {
      const r = await api.applyProfile(draft, cwd)
      setStatus(
        t('ui.filesWritten', { count: r.written?.length ?? 0, backups: r.backups?.length ?? 0 }),
      )
      setUndoPoint(r.point ?? null)
      const fresh = await api.resolveProfile(draft, cwd)
      if (token === buildSeq.current) {
        setResult(fresh)
        setStale(false)
      }
    } catch (e) {
      setStatus(t('ui.applyRejected', { message: (e as Error).message }))
    } finally {
      setBusy(false)
    }
  }

  /**
   * Rolling back the last write. Same order as the probe: the rescan clears the
   * status line, so the message has to be set after it, not before.
   */
  const doRestore = async (id: string) => {
    setBusy(true)
    setUndoPoint(null)
    try {
      const r = await api.restoreBackup(id)
      // A point is a write into a project or a profile save, and only the second
      // kind changes the list on the left. Re-reading it is cheap; not doing it
      // leaves a rolled-back profile showing its new version until a reload.
      const touched = [...r.restored, ...r.deleted]
      if (touched.some((f) => f.endsWith(`${draft.id}.json`))) {
        const fresh = await api.getProfiles()
        setProfiles(fresh)
        const mine = fresh.find((p) => p.id === draft.id)
        // The undo put this very file back, so the draft that describes it is
        // now a description of something else.
        if (mine) setDraft(mine)
      }
      await scan(cwd)
      setStatus(t('ui.undone', { restored: r.restored.length, deleted: r.deleted.length }))
    } catch (e) {
      setStatus((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const doProbe = async (name: string) => {
    setStatus(t('ui.probeStarting', { name }))
    setBusy(true)
    try {
      const { cost } = await api.probeMcp(name)
      // Rescan first: `scan` opens with setStatus(''), so a message set before it
      // is wiped on the next tick — the old order showed the user nothing at all.
      await scan(cwd)
      setStatus(t('ui.probeResult', { name, tokens: cost.tokens ?? '—', note: cost.note }))
    } catch (e) {
      setStatus(t('ui.probeFailed', { message: (e as Error).message }))
    } finally {
      setBusy(false)
    }
  }

  /**
   * An install is a CLI action — our part afterwards is to notice it: rescan,
   * and add the plugin to the profile being edited.
   */
  const afterInstall = async (id?: string) => {
    if (id?.includes('@')) {
      patch({
        plugins: {
          enabled: [...new Set([...draft.plugins.enabled, id])],
          disabled: draft.plugins.disabled.filter((x) => x !== id),
        },
      })
    }
    await scan(cwd)
  }

  const loaded = useMemo(() => profiles.find((p) => p.id === draft.id) ?? null, [profiles, draft.id])

  useEffect(() => {
    localStorage.setItem('cc-cols', JSON.stringify(cols))
  }, [cols])

  useEffect(() => {
    localStorage.setItem('cc-rail', String(railW))
  }, [railW])

  /** How far a pane may grow before it eats into the third pane's room. */
  const maxCol = (i: 0 | 1, from: [number, number]): number => {
    const room = (colsRef.current?.clientWidth ?? 1440) - HANDLES - RESULT_MIN
    const other = i === 0 ? from[1] : from[0]
    return Math.max(MIN[i], room - other)
  }

  /** Keeps a pane >= its minimum and leaves the third pane its 320px. */
  const clampCol = (i: 0 | 1, next: [number, number]): [number, number] => {
    next[i] = Math.min(Math.max(next[i], MIN[i]), maxCol(i, next))
    return next
  }

  // The widths are stored in px because they are the user's, and that is exactly
  // why a shrinking window used to push the third pane past the edge: at 900px
  // wide it was gone, with no scrollbar and no drag that could bring it back —
  // a drag clamps against the window it is already inside. Re-clamping on every
  // resize keeps the panes in view while the minimums still fit; below that the
  // column strip scrolls rather than hides (see `.cols`).
  useEffect(() => {
    const el = colsRef.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => {
      setCols(([a, b]) => {
        const next = clampCol(0, clampCol(1, [a, b]))
        // Same numbers, new array: returning it would re-render on every resize
        // event for nothing.
        return next[0] === a && next[1] === b ? [a, b] : next
      })
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // Pointer capture keeps move events coming to the handle even when the cursor
  // outruns it — otherwise a fast drag drops the pane mid-gesture.
  const startDrag = (i: 0 | 1) => (e: React.PointerEvent<HTMLDivElement>) => {
    drag.current = { i, startX: e.clientX, from: [...cols] }
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {
      /* capture is best-effort: without it the drag still tracks while the
         cursor stays over the 4px handle */
    }
  }

  const moveDrag = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = drag.current
    if (!d) return
    const next: [number, number] = [...d.from]
    next[d.i] = d.from[d.i] + (e.clientX - d.startX)
    setCols(clampCol(d.i, next))
  }

  const endDrag = (e: React.PointerEvent<HTMLDivElement>) => {
    drag.current = null
    railDrag.current = null
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId)
    }
  }

  /** The rail's handle: one width instead of a pair, same gesture as the panes. */
  const startRailDrag = (e: React.PointerEvent<HTMLDivElement>) => {
    railDrag.current = { startX: e.clientX, from: railW }
    try {
      e.currentTarget.setPointerCapture(e.pointerId)
    } catch {
      /* same best-effort capture as the pane handles */
    }
  }

  const moveRailDrag = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = railDrag.current
    if (!d) return
    setRailW(clampRail(d.from + (e.clientX - d.startX)))
  }

  const nudgeRail = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return
    e.preventDefault()
    setRailW(clampRail(railW + (e.key === 'ArrowRight' ? 16 : -16)))
  }

  /** The same handle, reachable without a mouse. */
  const nudge = (i: 0 | 1) => (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return
    e.preventDefault()
    const next: [number, number] = [...cols]
    next[i] = cols[i] + (e.key === 'ArrowRight' ? 16 : -16)
    setCols(clampCol(i, next))
  }

  const handle = (i: 0 | 1) => (
    <div
      className="handle"
      role="separator"
      aria-orientation="vertical"
      tabIndex={0}
      title={t('ui.resize')}
      aria-label={t('ui.resize')}
      /* A focusable separator is a range widget: without these it is announced
         as an unnamed 0–100 slider sitting at 50, which is not what it is. */
      aria-valuenow={cols[i]}
      aria-valuemin={MIN[i]}
      aria-valuemax={maxCol(i, cols)}
      onPointerDown={startDrag(i)}
      onPointerMove={moveDrag}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={nudge(i)}
    />
  )

  /**
   * What ⌘K offers. Built on every render rather than memoised: the closures
   * read the current draft, and a cached list would toggle yesterday's state.
   */
  const paletteItems = (): PaletteItem[] => {
    const items: PaletteItem[] = []
    const all = inventory?.entities ?? []
    // "all" is a mode in the profile, not a list: the toggles need the names themselves.
    const allMcp = [...new Set(all.filter((e) => e.kind === 'mcp').map((e) => e.name))]
    for (const e of all) {
      if (e.kind === 'mcp') {
        const on = isMcpOn(draft, e.name, e.offByFile)
        items.push({
          key: `mcp:${e.name}`,
          kind: t('inv.kindMcp'),
          label: e.name,
          note: t(on ? 'ui.chipOn' : 'ui.chipOff'),
          run: () => patch(toggleMcp(draft, e.name, allMcp, deniedMcpNames)),
        })
      } else if (e.kind === 'plugin') {
        // The row on the left reads the profile, which may have overridden the
        // disk — the palette has to say the same thing, and flip that.
        const on = isPluginOn(draft, e.id, e.enabled)
        items.push({
          key: `plugin:${e.id}`,
          kind: t('inv.kindPlugin'),
          label: e.id,
          note: t(on ? 'ui.chipOn' : 'ui.chipOff'),
          run: () => patch(togglePlugin(draft, e.id, !on, e.enabled)),
        })
      } else if (e.kind === 'skill' && !e.plugin) {
        // A plugin's skill has no switch here for the same reason it has no box on
        // the left: the entry would decide nothing. The plugin's own item, above,
        // is what turns it.
        const on = isSkillOn(draft, e.name, e.offByFile)
        items.push({
          key: `skill:${e.name}`,
          kind: t('inv.kindSkill'),
          label: e.name,
          note: t(on ? 'ui.chipOn' : 'ui.chipOff'),
          run: () => patch(toggleSkill(draft, e.name, e.offByFile)),
        })
      }
    }
    for (const p of profiles) {
      items.push({
        key: `profile:${p.id}`,
        kind: t('ui.kindProfile'),
        label: p.title || p.id,
        run: () => {
          loadProfile(p)
          setStatus(t('ui.profileLoaded', { title: p.title || p.id }))
        },
      })
    }
    return items
  }

  const views: { id: View; title: string; label?: string; icon: ReactNode }[] = [
    { id: 'settings', title: t('ui.settings'), icon: <IconSliders /> },
    // Two registries, two destinations: they are different places, not two tabs
    // of one place, so each gets its own row.
    { id: 'catalogPlugins', title: t('inv.netTitle'), label: t('cat.plugins'), icon: <IconGrid /> },
    { id: 'catalogMcp', title: t('inv.mcpRegistryTitle'), label: t('cat.mcp'), icon: <IconPlug /> },
    { id: 'stats', title: t('stats.title'), icon: <IconBars /> },
    // Beside the statistics and not inside them: that page counts what the sessions
    // cost, this one what they called, and the two are read for different reasons.
    { id: 'usage', title: t('usage.title'), icon: <IconBolt size={15} /> },
    { id: 'history', title: t('ui.historyTitle'), label: t('ui.history'), icon: <IconClock /> },
    // The constructor: what this build carries and what the machine answers about
    // it, with a switch per row. Last, because it is the least used one.
    { id: 'modules', title: t('mod.title'), icon: <IconPuzzle /> },
    // One row per module that is switched on. The rail is where the choice shows: a
    // module nobody turned on takes no space in it. Deliberately not "and whose tool
    // is here" — that answer costs a process, and what a module's page does about a
    // missing tool is that page's business, not the rail's.
    ...(mods ?? [])
      .filter((m) => m.enabled)
      .map((m) => ({ id: `mod:${m.id}` as View, title: t(m.titleKey), icon: moduleIcon(m.id) })),
  ]

  return (
    <div className="app">
      {/* The rail: every place this app can take you, down the left edge. They
          are destinations, not settings — the topbar's controls all act on the
          project in the path field, and these have nothing to do with it. The lit
          row is where you are, and the only way back is one of these rows. */}
      <nav className="rail" style={{ width: railW }} data-label={railW >= RAIL_LABEL ? 'on' : 'off'}>
        {views.map((v) => (
          <button
            key={v.id}
            className={`btn icon${view === v.id ? ' on' : ''}`}
            title={v.title}
            aria-label={v.title}
            aria-pressed={view === v.id}
            onClick={() => setView(v.id)}
          >
            {v.icon}
            <span className="rail-label">{v.label ?? v.title}</span>
          </button>
        ))}
      </nav>

      {/* Widened like the panes, and only useful widened — at 48px it is an icon
          strip and there is nothing to drag for. */}
      <div
        className="handle"
        role="separator"
        aria-orientation="vertical"
        tabIndex={0}
        title={t('ui.resize')}
        aria-label={t('ui.resize')}
        aria-valuenow={railW}
        aria-valuemin={RAIL_MIN}
        aria-valuemax={RAIL_MAX}
        onPointerDown={startRailDrag}
        onPointerMove={moveRailDrag}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onKeyDown={nudgeRail}
      />

      <div className="main">
        <header className="topbar">
          <div className="brand">
            claude-lineup <span>/ {t('ui.tagline')}</span>
          </div>
          {/* Beside the brand because it is about the work, not about the window:
              every click in the panes moves this number, and a figure that moves
              where nobody is looking may as well not move. */}
          <ContextMeter inventory={inventory} profile={draft} />
          <div className="grow" />
          {/* The path field and Reload act on the project in the panes. The other
              three destinations have no project of their own: leaving them up
              would offer a control that does nothing where you are standing. */}
          {view === 'settings' ? (
            <>
              <CwdField value={cwd} onChange={setCwd} onStatus={setStatus} onBusy={setBusy} width={380} />
              <button className="btn" onClick={() => void scan(cwd)} disabled={busy}>
                {t('ui.reload')}
              </button>
            </>
          ) : null}
          <button
            className="btn icon"
            title={t('ui.theme')}
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
          >
            {theme === 'dark' ? <IconMoon /> : <IconSun />}
          </button>
          <LangToggle />
          <button className="btn" title={t('ui.search')} onClick={() => setPaletteOpen(true)}>
            {MOD}K
          </button>
        </header>

        {/* Wherever the action was, the answer shows here. The message used to
            live in the result pane's foot, which is two panes away from Save
            profile — the one control that raises most of them. */}
        {status || undoPoint ? (
          <div className="statusbar" role="status">
            {status}
            {/* The undo of the write that just happened. It disappears with the
                next recompute, so it can never point at a file that moved on. */}
            {undoPoint && (
              <>
                <span className="grow" />
                <button
                  className="btn"
                  disabled={busy}
                  title={t('ui.historyTitle')}
                  onClick={() => void doRestore(undoPoint)}
                >
                  {t('ui.undo')}
                </button>
              </>
            )}
          </div>
        ) : null}


        {/* The workspace is one destination among four: on any other the panes
            stand down rather than float under a window. They stay mounted but
            hidden — the column widths and their resize observer belong to the
            user's window, and unmounting them would re-measure and re-clamp on
            every visit. */}
        <div
          className="cols"
          ref={colsRef}
          hidden={view !== 'settings'}
          style={{ gridTemplateColumns: `${cols[0]}px 4px ${cols[1]}px 4px minmax(320px, 1fr)` }}
        >
          <InventoryPane
            inventory={inventory}
            profile={draft}
            busy={busy}
            onChange={patch}
            onProbe={(n) => void doProbe(n)}
            onOpen={(kind, name, file) => setOpenCard({ kind, name, ...(file ? { file } : {}) })}
            onRule={(rules, state) => patch(setRules(draft, rules, state))}
          />
          {handle(0)}
          <ProfilePane
            draft={draft}
            profiles={profiles}
            onChange={patch}
            onNew={() =>
              loadProfile({
                ...EMPTY,
                id: `profile-${profiles.length + 1}`,
                title: t('ui.newProfile'),
              })
            }
            onLoad={(id) => {
              const p = profiles.find((x) => x.id === id)
              if (p) loadProfile(p)
            }}
            onDelete={async (id) => {
              await api.deleteProfile(id)
              setProfiles(await api.getProfiles())
              if (draft.id === id) loadProfile({ ...EMPTY, title: t('ui.newProfile') })
            }}
            onSave={() => void doSaveProfile()}
            busy={busy}
          />
          {handle(1)}
          <ResultPane
            profile={draft}
            result={result}
            stale={stale}
            busy={busy}
            cwd={cwd}
            onResolve={() => void doResolve()}
            onApply={() => void doApply()}
          />
        </div>

        {paletteOpen ? (
          <CommandPalette
            items={paletteItems()}
            onClose={() => setPaletteOpen(false)}
            onApply={() => void doApply()}
          />
        ) : null}

        {view === 'catalogPlugins' || view === 'catalogMcp' ? (
          <CatalogOverlay
            // A key per destination: the two registries share nothing — not the
            // query, not the install plan, not the open card.
            key={view}
            source={view === 'catalogPlugins' ? 'plugins' : 'mcp'}
            cwd={cwd}
            onCwd={setCwd}
            busy={busy}
            onBusy={setBusy}
            onStatus={setStatus}
            onInstalled={afterInstall}
            // Escape is the other way back, and it goes where the rail would.
            onClose={() => setView('settings')}
          />
        ) : null}

        {view === 'history' ? (
          <BackupsOverlay
            busy={busy}
            onStatus={setStatus}
            onRestore={doRestore}
            // A point that no longer exists cannot be rolled back, and the
            // statusbar button would offer it anyway — it is one click away.
            onForget={(ids) => setUndoPoint((p) => (p && ids.includes(p) ? null : p))}
            onClose={() => setView('settings')}
          />
        ) : null}

        {view === 'stats' ? <StatsPage /> : null}

        {view === 'modules' ? (
          <ModulesPage modules={mods ?? []} onChange={setMods} onStatus={setStatus} />
        ) : null}

        {/* Whatever the id is, the view is the module's own — the core only carries
            the route to it. Mounted only for a module that is switched on: a switched-off
            module's routes answer 409, and the core does not ask a page nobody can reach. */}
        {view.startsWith('mod:') && (mods ?? []).some((m) => m.enabled && m.id === view.slice(4))
          ? <ModuleView id={view.slice(4)} />
          : null}

        {view === 'usage' ? <UsagePage /> : null}

        {openCard ? (
          <div className="scrim" onClick={() => setOpenCard(null)}>
            <div className="catalog only" onClick={(e) => e.stopPropagation()}>
              <Card
                kind={openCard.kind}
                name={openCard.name}
                cwd={cwd}
                {...(openCard.file ? { file: openCard.file } : {})}
                busy={busy}
                onClose={() => setOpenCard(null)}
                // Only this card writes to the draft: the catalog's own card, opened
                // for a plugin that is not installed yet, has no profile behind it.
                {...(openCard.kind === 'skill'
                  ? {
                      // What the profile says, or — saying nothing — what the file
                      // does. A state picked here is written against that file state,
                      // so "on" means on rather than the removal of a key the file
                      // never had.
                      skillState:
                        draft.skills.states[openCard.name] ??
                        ((skillOffByFile(openCard.name) ? 'off' : 'on') as SkillState),
                      onSkillState: (s: SkillState) =>
                        patch(
                          setSkillState(draft, openCard.name, s, skillOffByFile(openCard.name)),
                        ),
                    }
                  : {})}
                {...(openCard.kind === 'mcp'
                  ? {
                      ruleStates: draft.permissions.rules,
                      // What the file denies and the profile has no opinion about:
                      // the per-tool rules live in `permissions.deny` and the
                      // inventory's own rows cannot carry them.
                      fileRuleStates: inventory?.fileRules ?? {},
                      onRule: (rules: string[], state: RuleState) =>
                        patch(setRules(draft, rules, state)),
                      // The card toggles servers the same way the rows on the left do:
                      // "all" is a mode, so the toggle needs the inventory's names.
                      mcpOn: isMcpOn(draft, openCard.name, mcpOffByFile(openCard.name)),
                      onMcp: () =>
                        patch(
                          toggleMcp(draft, openCard.name, allMcpNames, deniedMcpNames),
                        ),
                    }
                  : {})}
              />
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )
}
