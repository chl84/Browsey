import { afterEach, beforeEach } from 'vitest'

type UnhandledEvent = {
  type: 'error' | 'rejection'
  message: string
}

type GlobalWithHarness = typeof globalThis & {
  process?: {
    on: (
      event: 'unhandledRejection' | 'uncaughtException',
      listener: (value: unknown) => void,
    ) => void
  }
  __BROWSEY_TEST_HARNESS__?: {
    installed: boolean
    unhandledEvents: UnhandledEvent[]
  }
}

const globalHarness = globalThis as GlobalWithHarness

const installUnhandledEventHarness = () => {
  if (globalHarness.__BROWSEY_TEST_HARNESS__?.installed) {
    return globalHarness.__BROWSEY_TEST_HARNESS__
  }

  const state = {
    installed: true,
    unhandledEvents: [] as UnhandledEvent[],
  }

  globalHarness.process?.on('unhandledRejection', (reason: unknown) => {
    const message = reason instanceof Error ? reason.stack ?? reason.message : String(reason)
    state.unhandledEvents.push({ type: 'rejection', message })
  })

  globalHarness.process?.on('uncaughtException', (error: unknown) => {
    const message = error instanceof Error ? error.stack ?? error.message : String(error)
    state.unhandledEvents.push({ type: 'error', message })
  })

  if (typeof window !== 'undefined') {
    window.addEventListener('error', (event) => {
      const error = event.error
      const message =
        error instanceof Error
          ? error.stack ?? error.message
          : event.message || 'Unknown window error'
      state.unhandledEvents.push({ type: 'error', message })
    })

    window.addEventListener('unhandledrejection', (event) => {
      const reason = event.reason
      const message = reason instanceof Error ? reason.stack ?? reason.message : String(reason)
      state.unhandledEvents.push({ type: 'rejection', message })
    })
  }

  globalHarness.__BROWSEY_TEST_HARNESS__ = state
  return state
}

const harness = installUnhandledEventHarness()

beforeEach(() => {
  harness.unhandledEvents.length = 0
})

afterEach(() => {
  if (harness.unhandledEvents.length === 0) {
    return
  }
  const details = harness.unhandledEvents
    .map((event, index) => `${index + 1}. ${event.type}: ${event.message}`)
    .join('\n')
  harness.unhandledEvents.length = 0
  throw new Error(`Unhandled runtime events escaped the test:\n${details}`)
})
