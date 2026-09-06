import { useRef } from "react"

// Inline-edit fields commit on blur. Escape cancels by flipping isEditing off in the parent,
// which unmounts the field — and unmounting a focused element fires a native blur first, so
// the same onBlur handler would otherwise re-commit the value the user just tried to discard.
// cancel() marks that forced blur so commitUnlessCancelled() skips it exactly once.
export function useCommitGuard() {
  const cancelling = useRef(false)

  function cancel(onCancel: () => void) {
    cancelling.current = true
    onCancel()
  }

  function commitUnlessCancelled(commit: () => void) {
    if (cancelling.current) {
      cancelling.current = false
      return
    }
    commit()
  }

  return { cancel, commitUnlessCancelled }
}
