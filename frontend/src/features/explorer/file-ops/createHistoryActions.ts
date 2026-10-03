import { getErrorMessage } from '@/shared/lib/error'

type Deps = {
  undo: () => Promise<void>
  redo: () => Promise<void>
  refresh: () => Promise<void>
  showToast: (message: string, duration?: number) => void
}

export const createHistoryActions = (deps: Deps) => {
  let busy = false

  const run = async (operation: 'undo' | 'redo'): Promise<boolean> => {
    // Consume repeat shortcuts without queuing further destructive operations.
    if (busy) return true
    busy = true
    const label = operation === 'undo' ? 'Undo' : 'Redo'
    try {
      let failed = false
      let operationError = ''
      try {
        await deps[operation]()
      } catch (error) {
        failed = true
        operationError = getErrorMessage(error)
      }
      let refreshError = ''
      try {
        // An error can follow partial removal/restoration; reconcile either way.
        await deps.refresh()
      } catch (error) {
        refreshError = getErrorMessage(error)
      }
      if (failed) {
        deps.showToast(
          `${label} failed: ${operationError}. Inspect affected paths before retrying.${refreshError ? ` Refresh also failed: ${refreshError}. Press F5 to refresh.` : ''}`,
          5000,
        )
      } else if (refreshError) {
        deps.showToast(`${label} completed, but refresh failed: ${refreshError}. Press F5 to refresh.`, 4000)
      } else {
        deps.showToast(label)
      }
      return !failed
    } finally {
      busy = false
    }
  }

  return { undo: () => run('undo'), redo: () => run('redo') }
}
