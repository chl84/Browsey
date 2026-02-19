import { invoke } from '@/shared/lib/tauri'

export type ContextMenuActionsArgs = {
  count: number
  kind: string
  starred: boolean
  view: string
  clipboardHasItems: boolean
  selectionPaths: string[]
}

export const fetchContextMenuActions = <T>(args: ContextMenuActionsArgs) =>
  invoke<T>('context_menu_actions', args)
