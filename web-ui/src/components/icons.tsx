interface IconProps {
  size?: number
}

const base = {
  viewBox: "0 0 24 24",
  fill: "none" as const,
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
}

export function UndoIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <polyline points="9 14 4 9 9 4" />
      <path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11" />
    </svg>
  )
}

export function RedoIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <polyline points="15 14 20 9 15 4" />
      <path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13" />
    </svg>
  )
}

export function SunIcon({ size = 16 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
    </svg>
  )
}

export function MoonIcon({ size = 16 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  )
}

export function GlobeIcon({ size = 16 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <circle cx="12" cy="12" r="10" />
      <path d="M2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20" />
    </svg>
  )
}

export function SaveIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
      <path d="M17 21v-8H7v8M7 3v5h8" />
    </svg>
  )
}

export function PlayIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <polygon points="6 3 20 12 6 21 6 3" />
    </svg>
  )
}

export function StopIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base} fill="currentColor">
      <rect x="5" y="5" width="14" height="14" rx="1.5" />
    </svg>
  )
}

export function FolderIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M3 6a1 1 0 0 1 1-1h4.5l2 2H20a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6z" />
    </svg>
  )
}

export function ChatIcon({ size = 22 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z" />
    </svg>
  )
}

export function CloseIcon({ size = 16 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M18 6 6 18M6 6l12 12" />
    </svg>
  )
}

export function SendIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M22 2 11 13M22 2l-7 20-4-9-9-4 20-7z" />
    </svg>
  )
}

export function AlignIcon({ size = 16 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <path d="M12 2v20" />
      <path d="M6 7h12M8 12h8M5 17h14" />
    </svg>
  )
}

export function KeyboardIcon({ size = 14 }: IconProps) {
  return (
    <svg width={size} height={size} {...base}>
      <rect x="2" y="6" width="20" height="12" rx="2" />
      <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h12" />
    </svg>
  )
}
