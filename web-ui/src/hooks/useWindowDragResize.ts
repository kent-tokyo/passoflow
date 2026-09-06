import { useEffect, useRef, type MouseEvent } from "react"

interface Props {
  onMove: (start: { x: number; y: number }, current: { x: number; y: number }) => void
  onEnd?: () => void
}

export function useWindowDragResize({ onMove, onEnd }: Props) {
  const cleanupRef = useRef<(() => void) | null>(null)

  const start = (event: MouseEvent<HTMLElement>, start: { x: number; y: number }) => {
    event.preventDefault()
    event.stopPropagation()
    cleanupRef.current?.()

    const handleMouseMove = (moveEvent: globalThis.MouseEvent) => {
      onMove(start, { x: moveEvent.clientX, y: moveEvent.clientY })
    }
    const handleMouseUp = () => {
      window.removeEventListener("mousemove", handleMouseMove)
      window.removeEventListener("mouseup", handleMouseUp)
      cleanupRef.current = null
      onEnd?.()
    }

    cleanupRef.current = handleMouseUp
    window.addEventListener("mousemove", handleMouseMove)
    window.addEventListener("mouseup", handleMouseUp)
  }

  useEffect(() => () => cleanupRef.current?.(), [])

  return start
}
