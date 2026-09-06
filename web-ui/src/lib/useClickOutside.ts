import { useEffect, useRef, type RefObject } from "react"

// Calls onOutside for any mousedown outside ref.current, while active. onOutside is read
// through a ref so callers can pass an inline closure without resubscribing on every render.
export function useClickOutside(ref: RefObject<HTMLElement | null>, active: boolean, onOutside: () => void) {
  const onOutsideRef = useRef(onOutside)
  onOutsideRef.current = onOutside

  useEffect(() => {
    if (!active) return
    const onMouseDown = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) onOutsideRef.current()
    }
    window.addEventListener("mousedown", onMouseDown)
    return () => window.removeEventListener("mousedown", onMouseDown)
  }, [active, ref])
}
