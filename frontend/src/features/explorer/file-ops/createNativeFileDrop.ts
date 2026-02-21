import { writable } from 'svelte/store'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'

export type NativeDropState = {
  hovering: boolean
  position: { x: number; y: number } | null
}

type Options = {
  onDrop?: (paths: string[]) => void | Promise<void>
}

export const createNativeFileDrop = (options: Options = {}) => {
  const hovering = writable(false)
  const position = writable<{ x: number; y: number } | null>(null)
  let unlisten: UnlistenFn | null = null

  const start = async () => {
    if (unlisten) return
    const webview = await getCurrentWebview()
    unlisten = await webview.onDragDropEvent((event) => {
      const payload = event.payload
      if (payload.type === 'over') {
        hovering.set(true)
        position.set(payload.position ?? null)
      } else if (payload.type === 'drop') {
        hovering.set(false)
        position.set(null)
        options.onDrop?.(payload.paths ?? [])
      } else {
        hovering.set(false)
        position.set(null)
      }
    })
  }

  const stop = async () => {
    if (unlisten) {
      await unlisten()
      unlisten = null
    }
    hovering.set(false)
    position.set(null)
  }

  return { hovering, position, start, stop }
}
