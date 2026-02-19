import { invoke } from '@/lib/tauri'

export const storeColumnWidths = (widths: number[]) =>
  invoke<void>('store_column_widths', { widths })

export const loadSavedColumnWidths = () =>
  invoke<number[] | null>('load_saved_column_widths')
