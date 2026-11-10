/**
 * The two glyphs a font cannot be trusted with.
 *
 * `☾` and `⚡` both carry an emoji presentation, and on Windows Segoe UI Emoji
 * wins the fallback: the shell drew a coloured crescent in the topbar and a
 * coloured bolt on every MCP row, among otherwise monochrome line work. A path
 * has one presentation on every machine, so the UI keeps its single voice.
 *
 * 13px because that is the size of the text these sit next to, and
 * `currentColor` so they follow the button's own state, hover included.
 */
interface IconProps {
  size?: number
}

const box = (size: number) => ({
  width: size,
  height: size,
  viewBox: '0 0 16 16',
  fill: 'currentColor',
  'aria-hidden': true,
})

export function IconMoon({ size = 13 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path d="M13.4 10.3A5.9 5.9 0 0 1 5.7 2.6a5.9 5.9 0 1 0 7.7 7.7Z" />
    </svg>
  )
}

export function IconSun({ size = 13 }: IconProps) {
  return (
    <svg {...box(size)}>
      <circle cx="8" cy="8" r="3.1" />
      {/* Eight rays as one path: a stroked ring plus a disc read as a target, not a sun. */}
      <path
        d="M8 0.6v1.9M8 13.5v1.9M0.6 8h1.9M13.5 8h1.9M2.8 2.8l1.3 1.3M11.9 11.9l1.3 1.3M13.2 2.8l-1.3 1.3M4.1 11.9l-1.3 1.3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
    </svg>
  )
}

/** Settings: two sliders. The screen behind the other three rail rows. */
export function IconSliders({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path
        d="M2 4.6h12M2 11.4h12"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
      <circle cx="5.6" cy="4.6" r="1.9" />
      <circle cx="10.4" cy="11.4" r="1.9" />
    </svg>
  )
}

/** The catalog: four panes, the shape the window itself has. */
export function IconGrid({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path d="M2 2h5.2v5.2H2zM8.8 2H14v5.2H8.8zM2 8.8h5.2V14H2zM8.8 8.8H14V14H8.8z" />
    </svg>
  )
}

/** The MCP catalog: a plug — a server is something the harness connects to. */
export function IconPlug({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path
        d="M5.4 1.3v3.5M10.6 1.3v3.5M3.6 4.8h8.8v5.4H3.6zM8 10.2v4.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  )
}

/** The write history: a clock, because a restore point is a moment. */
export function IconClock({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <circle cx="8" cy="8" r="5.9" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M8 4.6V8.2l2.3 1.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

/** The statistics: three bars stepping up, a chart rather than a clock face. */
export function IconBars({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path d="M2 9h2.6v5H2zM6.2 5.4h2.6V14H6.2zM10.4 2h2.6v12h-2.6z" />
    </svg>
  )
}

export function IconBolt({ size = 13 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path d="M9.8 0.8 3.4 9.2h3.4l-0.8 6 6.4-8.4H9l0.8-6Z" />
    </svg>
  )
}

/**
 * The module catalog: three tiles placed and one dashed — a part that fits the
 * whole, and the empty slot it comes from. Deliberately the same four-pane shape
 * as the plugin catalog, because it is the same kind of place.
 */
export function IconPuzzle({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path d="M2 2h5.2v5.2H2zM8.8 2H14v5.2H8.8zM2 8.8h5.2V14H2z" />
      <path
        d="M8.8 8.8H14V14H8.8z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeDasharray="2 2"
      />
    </svg>
  )
}

/** A knowledge base: a book — it is read, and it is written into. */
export function IconBook({ size = 15 }: IconProps) {
  return (
    <svg {...box(size)}>
      <path
        d="M8 3.5C6.6 2.4 4.8 2 2.4 2v10.5c2.4 0 4.2.4 5.6 1.5 1.4-1.1 3.2-1.5 5.6-1.5V2c-2.4 0-4.2.4-5.6 1.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinejoin="round"
      />
      <path d="M8 3.5V14" fill="none" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  )
}
