import { useContext } from "react"
import { RunningNodeContext } from "./RunningNodeContextBase"

export function useIsRunningNode(nodeId: string): boolean {
  const runningNodeId = useContext(RunningNodeContext)
  return runningNodeId !== null && runningNodeId === nodeId
}
