import { writable } from 'svelte/store'
import type { Entry } from '../model/types'

export type ContextAction = {
  id: string
  label: string
  shortcut?: string
  dangerous?: boolean
  children?: ContextAction[]
}

type ContextMenuState = {
  open: boolean
  x: number
  y: number
  actions: ContextAction[]
  entry: Entry | null
}

type BlankMenuState = {
  open: boolean
  x: number
  y: number
  actions: ContextAction[]
}

export const createContextMenus = () => {
  const contextMenu = writable<ContextMenuState>({
    open: false,
    x: 0,
    y: 0,
    actions: [],
    entry: null,
  })

  const blankMenu = writable<BlankMenuState>({
    open: false,
    x: 0,
    y: 0,
    actions: [],
  })

  const openContextMenu = (entry: Entry, actions: ContextAction[], x: number, y: number) => {
    contextMenu.set({ open: true, x, y, actions, entry })
  }

  const closeContextMenu = () => {
    contextMenu.update((s) => ({ ...s, open: false, entry: null }))
  }

  const openBlankContextMenu = (actions: ContextAction[], x: number, y: number) => {
    blankMenu.set({ open: true, x, y, actions })
  }

  const closeBlankContextMenu = () => {
    blankMenu.update((s) => ({ ...s, open: false, actions: [] }))
  }

  return {
    contextMenu,
    blankMenu,
    openContextMenu,
    closeContextMenu,
    openBlankContextMenu,
    closeBlankContextMenu,
  }
}
