import { invoke } from '@/shared/lib/tauri'

export type WindowControlPolicy = {
  minimize: boolean
  maximize: boolean
}

export const fetchWindowControlPolicy = () =>
  invoke<WindowControlPolicy>('get_window_control_policy')
