import { invoke } from '@/shared/lib/tauri'

export const ejectDrive = (path: string) => invoke<void>('eject_drive', { path })
