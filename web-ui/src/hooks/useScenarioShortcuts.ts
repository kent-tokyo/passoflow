import { useEffect, type MutableRefObject } from "react"
import type { XYPosition } from "reactflow"

interface Props {
  lastMousePositionRef: MutableRefObject<{ x: number; y: number } | null>
  handleSave: () => Promise<void> | void
  toggleFocusMode: () => void
  undo: () => void
  redo: () => void
  copySelectedNodes: () => void
  cutSelectedNodes: () => void
  pasteNodesAt: (position: XYPosition) => void
  screenToFlowPosition: (screenX: number, screenY: number) => XYPosition | null
}

export function useScenarioShortcuts({
  lastMousePositionRef,
  handleSave,
  toggleFocusMode,
  undo,
  redo,
  copySelectedNodes,
  cutSelectedNodes,
  pasteNodesAt,
  screenToFlowPosition,
}: Props) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Ctrl+S must work regardless of focus, while IME composition is excluded.
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s" && !event.isComposing) {
        event.preventDefault()
        void handleSave()
        return
      }

      const target = event.target as HTMLElement | null
      const isEditable = target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
      if (isEditable || !(event.ctrlKey || event.metaKey)) return

      if (event.key.toLowerCase() === "f" && event.shiftKey) {
        event.preventDefault()
        toggleFocusMode()
      } else if (event.key.toLowerCase() === "z" && event.shiftKey) {
        event.preventDefault()
        redo()
      } else if (event.key.toLowerCase() === "z") {
        event.preventDefault()
        undo()
      } else if (event.key.toLowerCase() === "y") {
        event.preventDefault()
        redo()
      } else if (event.key.toLowerCase() === "c") {
        event.preventDefault()
        copySelectedNodes()
      } else if (event.key.toLowerCase() === "x") {
        event.preventDefault()
        cutSelectedNodes()
      } else if (event.key.toLowerCase() === "v") {
        event.preventDefault()
        const mouse = lastMousePositionRef.current
        const position = mouse ? screenToFlowPosition(mouse.x, mouse.y) : null
        pasteNodesAt(position ?? { x: 250, y: 250 })
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [copySelectedNodes, cutSelectedNodes, handleSave, lastMousePositionRef, pasteNodesAt, redo, screenToFlowPosition, toggleFocusMode, undo])
}
