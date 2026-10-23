import type { ActivityApi } from '../hooks/createActivity'
import { createAppLifecycle } from '../hooks/createAppLifecycle'

type Params = {
  handleResize: () => void
  loadDefaultView: () => Promise<'list' | 'grid' | null>
  applyDefaultView: (view: 'list' | 'grid') => void
  loadShortcuts: () => Promise<any>
  applyShortcutBindings: (next: any[]) => void
  startNativeDrop: () => Promise<void>
  stopNativeDrop: () => Promise<void>
  fsLabel: (fs?: string) => string
  activityApi: Pick<ActivityApi, 'start' | 'reportProgress' | 'hideSoon' | 'cleanup' | 'cleanupAll'>
  activity: { set: (value: any) => void }
  showToast: (message: string, ms?: number) => void
  abortDirStats: () => void
  cleanupScrollHover: () => void
  viewObservers: { cleanup: () => void }
  cancelToggleViewModeRaf: () => void
  resetNewFileTypeHint: () => void
  cancelSearch: () => void
  stopDuplicateScan: (silent?: boolean) => Promise<void> | void
}

export const useExplorerPageLifecycle = (params: Params) => {
  const initLifecycle = createAppLifecycle({
    handleResize: params.handleResize,
    loadDefaultView: params.loadDefaultView,
    applyDefaultView: params.applyDefaultView,
    loadShortcuts: params.loadShortcuts,
    applyShortcutBindings: params.applyShortcutBindings,
    startNativeDrop: params.startNativeDrop,
    stopNativeDrop: params.stopNativeDrop,
    onMountStarted: (fs) => {
      void params.activityApi.start(`Connecting to ${params.fsLabel(fs)}…`, `mount-${fs ?? 'network'}`)
        .catch(() => {})
    },
    onMountDone: (fs, ok, outcome) => {
      const label =
        outcome === 'already_connected'
          ? `Already connected to ${params.fsLabel(fs)}`
          : outcome === 'connected'
            ? `Connected to ${params.fsLabel(fs)}`
            : outcome === 'failed'
              ? `Failed to connect to ${params.fsLabel(fs)}`
              : ok
                ? `Connected to ${params.fsLabel(fs)}`
                : `Failed to connect to ${params.fsLabel(fs)}`
      const event = `mount-${fs ?? 'network'}`
      params.activityApi.reportProgress(event, { bytes: 0, total: 0, phase: label })
      params.activityApi.hideSoon(event)
      void params.activityApi.cleanup(true, event).catch(() => {})
      if (outcome === 'already_connected') {
        params.showToast('Already connected', 1400)
      }
    },
    onErrorToast: params.showToast,
    onCleanup: () => {
      params.abortDirStats()
      params.cleanupScrollHover()
      params.viewObservers.cleanup()
    },
  })

  const handlePageDestroy = () => {
    params.cancelToggleViewModeRaf()
    params.resetNewFileTypeHint()
    params.cancelSearch()
    void params.activityApi.cleanupAll()
    void params.stopDuplicateScan(true)
  }

  return {
    initLifecycle,
    handlePageDestroy,
  }
}
