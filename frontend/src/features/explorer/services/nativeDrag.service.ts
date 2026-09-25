import { invoke } from '../../../shared/lib/tauri'

// Copy-only: the receiving application owns the transfer. Do not retain an IPC
// completion Channel in native GTK callbacks or delete the source on completion.
export const startNativeFileDrag = async (paths: string[]) => {
  if (paths.length === 0 || paths.some(path => path.startsWith('rclone://'))) return false
  try {
    await invoke<void>('start_native_file_drag', { paths })
    return true
  } catch (err) {
    console.error('native drag failed', err)
    return false
  }
}
