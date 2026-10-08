import { invoke } from '@tauri-apps/api/core'

const prefix = 'browsey-drag://cloud/'
export const cloudDragToken = (paths: string[]): string | null => {
  if (paths.length !== 1) return null
  const match = /^browsey-drag:\/\/cloud\/([a-f0-9]{64})$/.exec(paths[0])
  return match?.[1] ?? null
}
export const isCloudDragOffer = (paths: string[]) => paths.some(path => path.startsWith('browsey-drag:'))

export const prepareCloudDrag = (paths: string[]) => {
  const bytes = crypto.getRandomValues(new Uint8Array(32))
  const token = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')
  const ready = invoke<void>('prepare_cloud_drag', { token, paths: [...paths] })
  let released = false
  return {
    token,
    payload: prefix + token,
    ready,
    release: () => {
      if (released) return
      released = true
      // WebKit ends the source before the receiving native drop is delivered.
      // Let the receiver snapshot metadata first; never wait on the GTK thread.
      void ready.then(() => new Promise<void>(resolve => setTimeout(resolve, 2000)))
        .then(() => invoke('release_cloud_drag', { token })).catch(() => {})
    },
  }
}
export const resolveCloudDrag = (token: string, verify: boolean) =>
  invoke<string[]>('resolve_cloud_drag', { token, verify })
