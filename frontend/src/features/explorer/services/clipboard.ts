import { invoke } from '@tauri-apps/api/core'

export const copyPathsToSystemClipboard = (paths: string[], mode: 'copy' | 'cut' = 'copy') =>
  invoke<void>('copy_paths_to_system_clipboard', { paths, mode })

export const setClipboardCmd = (paths: string[], mode: 'copy' | 'cut') =>
  invoke<void>('set_clipboard_cmd', { paths, mode })

export const clearSystemClipboard = () =>
  invoke<void>('clear_system_clipboard')

export const pasteClipboardCmd = (
  dest: string,
  policy: 'rename' | 'overwrite' = 'rename',
  progressEvent?: string,
) => invoke<void>('paste_clipboard_cmd', { dest, policy, progressEvent })

export const pasteClipboardPreview = (dest: string) =>
  invoke<{ src: string; target: string; is_dir: boolean }[]>('paste_clipboard_preview', { dest })
