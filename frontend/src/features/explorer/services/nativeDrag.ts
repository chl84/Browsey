import { startDrag } from '@crabnebula/tauri-plugin-drag'

export const startNativeFileDrag = async (paths: string[], mode: 'copy' | 'move' = 'copy') => {
  if (!paths || paths.length === 0) return false
  try {
    // Use first path as preview icon; OS will substitute a default if unsuitable.
    await startDrag({
      item: paths,
      icon: paths[0],
      mode,
    })
    return true
  } catch (err) {
    console.error('native drag failed', err)
    return false
  }
}
