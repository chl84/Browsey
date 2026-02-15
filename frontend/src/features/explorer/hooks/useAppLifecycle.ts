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
    const cleanupFns: Array<() => void | Promise<void>> = []
    let disposed = false

    const runCleanup = (fn: () => void | Promise<void>) => {
      void Promise.resolve()
        .then(() => fn())
        .catch((err) => {
          console.error('Lifecycle cleanup failed', err)
        })
    }

    const registerCleanup = (fn: () => void | Promise<void>) => {
      if (disposed) {
        runCleanup(fn)
        return
      }
      cleanupFns.push(fn)
    }

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
      if (disposed) return
      deps.handleResize()
      window.addEventListener('resize', deps.handleResize)
      registerCleanup(() => window.removeEventListener('resize', deps.handleResize))

      window.addEventListener('error', handleError)
      window.addEventListener('unhandledrejection', handleRejection)
      registerCleanup(() => {
        window.removeEventListener('error', handleError)
        window.removeEventListener('unhandledrejection', handleRejection)
      })

      const prefView = await deps.loadDefaultView().catch(() => null)
      if (disposed) return
      if (prefView === 'list' || prefView === 'grid') {
        deps.applyDefaultView(prefView)
      }

      const savedShortcuts = await deps.loadShortcuts().catch((err) => {
        console.error('Failed to load shortcuts', err)
        return null
      })
      if (disposed) return
      if (savedShortcuts && savedShortcuts.length > 0) {
        deps.applyShortcutBindings(savedShortcuts)
      }

      await deps.startNativeDrop()
      if (disposed) {
        await deps.stopNativeDrop().catch((err) => {
          console.error('Failed to stop native drop', err)
        })
        return
      }
      registerCleanup(() => deps.stopNativeDrop())

      const unlistenMountStart = await listen<MountEvent>('mounting-started', (event) => {
        const { fs } = event.payload ?? {}
        deps.onMountStarted(fs)
      })
      if (disposed) {
        await unlistenMountStart()
        return
      }

      const unlistenMountDone = await listen<MountEvent>('mounting-done', (event) => {
        const { fs, ok } = event.payload ?? {}
        deps.onMountDone(fs, ok)
      })
      if (disposed) {
        await unlistenMountStart()
        await unlistenMountDone()
        return
      }

      registerCleanup(async () => {
        await unlistenMountStart()
        await unlistenMountDone()
      })
    }

    void setupCore()

    return () => {
      disposed = true
      const fns = cleanupFns.splice(0).reverse()
      fns.forEach(runCleanup)
      deps.onCleanup()
    }
  }
}
