import type { ScenarioSummary } from "../types/scenario"

export interface TreeFolderNode {
  type: "folder"
  name: string
  path: string
  children: TreeNode[]
}

export interface TreeFileNode {
  type: "file"
  name: string
  path: string
  title: string
}

export type TreeNode = TreeFolderNode | TreeFileNode

/** Build a nested folder/file tree from the flat lists the backend returns. */
export function buildScenarioTree(summaries: ScenarioSummary[], folders: string[]): TreeNode[] {
  const root: TreeFolderNode = { type: "folder", name: "", path: "", children: [] }

  const getOrCreateFolder = (path: string): TreeFolderNode => {
    if (path === "") return root
    let current = root
    let currentPath = ""
    for (const part of path.split("/")) {
      currentPath = currentPath ? `${currentPath}/${part}` : part
      let next = current.children.find(
        (c): c is TreeFolderNode => c.type === "folder" && c.name === part,
      )
      if (!next) {
        next = { type: "folder", name: part, path: currentPath, children: [] }
        current.children.push(next)
      }
      current = next
    }
    return current
  }

  for (const folder of folders) getOrCreateFolder(folder)

  for (const summary of summaries) {
    const parts = summary.filename.split("/")
    const name = parts[parts.length - 1]
    const folder = getOrCreateFolder(parts.slice(0, -1).join("/"))
    folder.children.push({ type: "file", name, path: summary.filename, title: summary.title })
  }

  const sortChildren = (node: TreeFolderNode) => {
    node.children.sort((a, b) => {
      if (a.type !== b.type) return a.type === "folder" ? -1 : 1
      return a.name.localeCompare(b.name, "ja")
    })
    node.children.forEach((child) => {
      if (child.type === "folder") sortChildren(child)
    })
  }
  sortChildren(root)

  return root.children
}
