import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createExplorerStores } from './stores'

describe('createExplorerStores loading', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-02-28T12:00:00Z'))
  })

  it('does not show loading for very fast work', () => {
    const { loading } = createExplorerStores()

    loading.set(true)
    vi.advanceTimersByTime(100)
    loading.set(false)
    vi.runAllTimers()

    expect(get(loading)).toBe(false)
  })

  it('keeps loading visible briefly once shown', () => {
    const { loading } = createExplorerStores()

    loading.set(true)
    vi.advanceTimersByTime(150)
    expect(get(loading)).toBe(true)

    vi.advanceTimersByTime(20)
    loading.set(false)
    expect(get(loading)).toBe(true)

    vi.advanceTimersByTime(159)
    expect(get(loading)).toBe(true)

    vi.advanceTimersByTime(1)
    expect(get(loading)).toBe(false)
  })
})
