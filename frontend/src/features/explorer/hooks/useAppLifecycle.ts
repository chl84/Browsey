import { listen } from '@tauri-apps/api/event'
import type { ShortcutBinding } from '../../shortcuts/keymap'

type ViewMode = 'list' | 'grid'
type MountEvent = { path: string; fs?: string; ok?: boolean; duration_ms?: number }

type Deps = {
  handleResize: () => void
  loadDefaultView: () => Promise<ViewMode | null>
  applyDefaultView: (view: ViewMode) => void
  loadShortcuts: () => Promise<ShortcutBinding[] | null>
  applyShortcutBindings: (next: ShortcutBinding[]) => void
  startNativeDrop: () => Promise<void>
  stopNativeDrop: () => Promise<void>
  onMountStarted: (fs?: string) => void
  onMountDone: (fs?: string, ok?: boolean) => void
  onErrorToast: (message: string) => void
  onCleanup: () => void
}

export const createAppLifecycle = (deps: Deps) => {
  return () => {
    const cleanupFns: Array<() => void> = []

    const handleError = (event: ErrorEvent) => {
      const msg = event.error instanceof Error ? event.error.message : event.message ?? 'Unknown error'
      console.error('Unhandled error', event)
      deps.onErrorToast(`Error: ${msg}`)
    }
    const handleRejection = (event: PromiseRejectionEvent) => {
      const reason = event.reason
      const msg = reason instanceof Error ? reason.message : String(reason)
      console.error('Unhandled rejection', reason)
      deps.onErrorToast(`Error: ${msg}`)
    }

    const setupCore = async () => {
      deps.handleResize()
      window.addEventListener('resize', deps.handleResize)
      cleanupFns.push(() => window.removeEventListener('resize', deps.handleResize))

      window.addEventListener('error', handleError)
      window.addEventListener('unhandledrejection', handleRejection)
      cleanupFns.push(() => {
        window.removeEventListener('error', handleError)
        window.removeEventListener('unhandledrejection', handleRejection)
      })

      const prefView = await deps.loadDefaultView().catch(() => null)
      if (prefView === 'list' || prefView === 'grid') {
        deps.applyDefaultView(prefView)
      }

      const savedShortcuts = await deps.loadShortcuts().catch((err) => {
        console.error('Failed to load shortcuts', err)
        return null
      })
      if (savedShortcuts && savedShortcuts.length > 0) {
        deps.applyShortcutBindings(savedShortcuts)
      }

      await deps.startNativeDrop()
      cleanupFns.push(() => {
        void deps.stopNativeDrop()
      })

      const unlistenMountStart = await listen<MountEvent>('mounting-started', (event) => {
        const { fs } = event.payload ?? {}
        deps.onMountStarted(fs)
      })
      const unlistenMountDone = await listen<MountEvent>('mounting-done', (event) => {
        const { fs, ok } = event.payload ?? {}
        deps.onMountDone(fs, ok)
      })
      cleanupFns.push(() => {
        unlistenMountStart()
        unlistenMountDone()
      })
    }

    void setupCore()

    return () => {
      cleanupFns.forEach((fn) => fn())
      deps.onCleanup()
    }
  }
}
