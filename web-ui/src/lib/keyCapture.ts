// Maps a browser KeyboardEvent's physical key (event.code, layout-independent) to the
// key name pyautogui.KEYBOARD_KEYS expects, so a key the user presses can be recorded
// directly instead of picked from a dropdown.
const CODE_TO_PYAUTOGUI: Record<string, string> = {
  Enter: "enter",
  Escape: "esc",
  Backspace: "backspace",
  Tab: "tab",
  Space: "space",
  Delete: "delete",
  Insert: "insert",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  ControlLeft: "ctrlleft",
  ControlRight: "ctrlright",
  AltLeft: "altleft",
  AltRight: "altright",
  ShiftLeft: "shiftleft",
  ShiftRight: "shiftright",
  MetaLeft: "winleft",
  MetaRight: "winright",
  CapsLock: "capslock",
  PrintScreen: "prtsc",
  ScrollLock: "scrolllock",
  Pause: "pause",
  ContextMenu: "apps",
  NumLock: "numlock",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  NumpadAdd: "add",
  NumpadSubtract: "subtract",
  NumpadMultiply: "multiply",
  NumpadDivide: "divide",
  NumpadDecimal: "decimal",
  NumpadEnter: "enter",
}

const BARE_MODIFIER_KEYS = new Set([
  "ctrl",
  "ctrlleft",
  "ctrlright",
  "alt",
  "altleft",
  "altright",
  "shift",
  "shiftleft",
  "shiftright",
  "win",
  "winleft",
  "winright",
])

/** Map one physical key to its pyautogui name, or null if it has no direct equivalent. */
export function mapCodeToPyautogui(code: string): string | null {
  if (code in CODE_TO_PYAUTOGUI) return CODE_TO_PYAUTOGUI[code]
  const letterMatch = /^Key([A-Z])$/.exec(code)
  if (letterMatch) return letterMatch[1].toLowerCase()
  const digitMatch = /^Digit(\d)$/.exec(code)
  if (digitMatch) return digitMatch[1]
  const numpadMatch = /^Numpad(\d)$/.exec(code)
  if (numpadMatch) return `num${numpadMatch[1]}`
  const fnMatch = /^F(\d{1,2})$/.exec(code)
  if (fnMatch && Number(fnMatch[1]) <= 24) return `f${fnMatch[1]}`
  return null
}

/** Single-key capture, for press_key: the exact physical key that was pressed. */
export function captureKeyFromEvent(event: KeyboardEvent): string | null {
  return mapCodeToPyautogui(event.code)
}

/**
 * Chord capture, for hotkey: builds e.g. ['ctrl', 'shift', 'esc'] from held modifiers
 * plus the key that triggered the event, without double-counting a lone modifier press.
 */
export function captureChordFromEvent(event: KeyboardEvent): string[] {
  const keys: string[] = []
  if (event.ctrlKey) keys.push("ctrl")
  if (event.shiftKey) keys.push("shift")
  if (event.altKey) keys.push("alt")
  if (event.metaKey) keys.push("win")
  const mainKey = mapCodeToPyautogui(event.code)
  if (mainKey && !BARE_MODIFIER_KEYS.has(mainKey)) keys.push(mainKey)
  return keys
}
