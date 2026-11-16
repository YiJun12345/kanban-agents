import type { ReactNode } from 'react'

// Small line icons drawn as SVG, so they look the same on every computer
// (emoji and symbol characters depend on the fonts that happen to be installed)
const SHAPES = {
  arrow: <path d="M3 8h10M9 4l4 4-4 4" />,
  play: <path d="M5.5 3.5v9l7-4.5z" fill="currentColor" />,
  stop: <rect x="4.5" y="4.5" width="7" height="7" rx="1" fill="currentColor" />,
  pen: <path d="M3 13l.8-3.2 6.7-6.7a1.4 1.4 0 0 1 2 2L5.8 11.8 3 13zM9.5 4.5l2 2" />,
  star: (
    <path
      d="M8 2.2l1.7 3.6 3.9.5-2.9 2.7.8 3.9L8 11l-3.5 1.9.8-3.9-2.9-2.7 3.9-.5z"
      fill="currentColor"
    />
  ),
  commit: <path d="M1.5 8h3.5M11 8h3.5M11 8a3 3 0 1 1-6 0 3 3 0 0 1 6 0z" />,
  merge: (
    <path d="M4.5 5.5v5M4.5 5.5c0 3 2.5 3 5.5 3M6 4a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0zM6 12a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0zM13 8.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z" />
  ),
  alert: <path d="M8 2.5l6 10.5H2L8 2.5zM8 6.5v3M8 11.3v.2" />,
  flag: <path d="M4 14V2.5M4 3h7.5l-1.8 2.75L11.5 8.5H4" />,
  check: <path d="M3 8.5l3.2 3L13 4.5" />,
  close: <path d="M4 4l8 8M12 4l-8 8" />,
  plus: <path d="M8 3v10M3 8h10" />,
  chevron: <path d="M6 4l4 4-4 4" />,
  redo: <path d="M12.5 3v3.5H9M12.2 6.5A4.8 4.8 0 1 0 13 9.5" />,
} satisfies Record<string, ReactNode>

export type IconName = keyof typeof SHAPES

export function Icon({ name, size = 14 }: { name: IconName; size?: number }) {
  return (
    <svg
      className="icon-svg"
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {SHAPES[name]}
    </svg>
  )
}
