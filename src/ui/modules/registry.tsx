import type { ReactNode } from 'react'
import { IconBook, IconPuzzle } from '../components/Icon.js'
import { OpenkbView } from './OpenkbView.js'

/**
 * What each module draws, by id — the UI's half of the registry.
 *
 * Static, and that is the honest edge of this design: the SERVER decides which
 * modules are visible and switchable, but a module's view is in the bundle either
 * way. A module listed here is a module whose code this build carries; a module the
 * server lists and this map does not know is still readable as a row and still
 * switchable, and opening its tab says the view is missing rather than pretending.
 * A third-party module shipped as a file would need a runtime import the build does
 * not do yet — ponytail: the honest boundary, not a hidden one.
 */
const VIEWS: Record<string, { icon: ReactNode; View: () => ReactNode }> = {
  openkb: { icon: <IconBook />, View: OpenkbView },
}

/** A module with no view of its own still gets the catalog's own glyph. */
export const moduleIcon = (id: string): ReactNode => VIEWS[id]?.icon ?? <IconPuzzle />

export function ModuleView({ id }: { id: string }) {
  const entry = VIEWS[id]
  if (!entry) return <div className="page" />
  const View = entry.View
  return <View />
}
