import { convertFileSrc, invoke as rawInvoke } from '@tauri-apps/api/core'

import { normalizeError } from './error'

export { convertFileSrc }

export const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  try {
    return await rawInvoke<T>(cmd, args)
  } catch (error) {
    throw normalizeError(error)
  }
}
