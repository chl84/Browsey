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
  activityApi: {
    clearNow: () => void
    hideSoon: () => void
    cleanup: (...args: any[]) => Promise<void> | void
  }
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
      params.activityApi.clearNow()
      params.activity.set({
        label: `Connecting to ${params.fsLabel(fs)}…`,
        percent: null,
        cancel: null,
        cancelling: false,
      })
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
      params.activity.set({ label, percent: null, cancel: null, cancelling: false })
      params.activityApi.hideSoon()
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
    params.activityApi.clearNow()
    void params.activityApi.cleanup()
    void params.stopDuplicateScan(true)
  }

  return {
    initLifecycle,
    handlePageDestroy,
  }
}
