import { writable } from 'svelte/store'
import { onDestroy } from 'svelte'
import { invoke } from '@tauri-apps/api/core'

export type DragState = {
  dragging: boolean
  paths: string[]
  target: string | null
  position: { x: number; y: number }
}

const normalizePath = (p: string) => {
  if (!p) return ''
  const withSlashes = p.replace(/\\/g, '/')
  const trimmed = withSlashes.replace(/\/+$/, '')
  if (trimmed.length === 0) {
    return withSlashes.startsWith('/') ? '/' : ''
  }
  return trimmed
}

const isSubPath = (parent: string, child: string) => {
  const normParentRaw = normalizePath(parent)
  const normChild = normalizePath(child)
  const normParent = normParentRaw.endsWith('/') ? normParentRaw : `${normParentRaw}/`
  return normChild === normParentRaw || normChild.startsWith(normParent)
}

type DragDropOptions = {
  getLabel?: (paths: string[]) => string
}

export const useDragDrop = (options: DragDropOptions = {}) => {
  const state = writable<DragState>({ dragging: false, paths: [], target: null, position: { x: 0, y: 0 } })

  const start = (paths: string[], event: DragEvent) => {
    if (!event.dataTransfer) return
    state.set({ dragging: true, paths, target: null, position: { x: event.clientX, y: event.clientY } })
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', paths.join('\n'))
    const labelText =
      options.getLabel?.(paths) ?? `${paths.length} item${paths.length === 1 ? '' : 's'}`

    // Minimal custom drag image for consistent visuals across views
    const ghost = document.createElement('div')
    ghost.style.position = 'absolute'
    ghost.style.pointerEvents = 'none'
    ghost.style.top = '-999px'
    ghost.style.left = '-999px'
    ghost.style.padding = '4px 8px'
    ghost.style.borderRadius = '0'
    ghost.style.background = 'var(--drag-ghost-bg)'
    ghost.style.color = 'var(--drag-ghost-text)'
    ghost.style.fontSize = '12px'
    ghost.style.fontWeight = '600'
    ghost.style.display = 'inline-flex'
    ghost.style.alignItems = 'center'
    ghost.style.gap = '6px'
    ghost.style.boxShadow = 'var(--shadow-drop)'
    const dot = document.createElement('span')
    dot.style.width = '8px'
    dot.style.height = '8px'
    dot.style.borderRadius = '0'
    dot.style.background = 'var(--drag-ghost-dot-bg)'
    ghost.appendChild(dot)
    const label = document.createElement('span')
    label.textContent = labelText
    ghost.appendChild(label)
    document.body.appendChild(ghost)
    event.dataTransfer.setDragImage(ghost, 0, 0)

    requestAnimationFrame(() => {
      document.body.removeChild(ghost)
    })
  }

  const end = () => {
    state.set({ dragging: false, paths: [], target: null, position: { x: 0, y: 0 } })
  }

  const canDropOn = (dragPaths: string[], targetPath: string) => {
    if (dragPaths.length === 0) return false
    return !dragPaths.some((p) => isSubPath(p, targetPath))
  }

  const setTarget = (target: string | null) => {
    state.update((s) => ({ ...s, target }))
  }

  const setPosition = (x: number, y: number) => {
    state.update((s) => ({ ...s, position: { x, y } }))
  }

  const move = async (paths: string[], dest: string) => {
    if (paths.length === 0) return
    await invoke('set_clipboard_cmd', { paths, mode: 'cut' })
    await invoke('paste_clipboard_cmd', { dest })
  }

  onDestroy(() => {
    end()
  })

  return { state, start, end, canDropOn, move, setTarget, setPosition }
}
