import { describe, expect, it, vi } from 'vitest'

import { DEFAULT_SETTINGS, type Settings } from '../settingsTypes'
import { createSettingsModalViewModel } from './useSettingsModalViewModel'

const buildDeps = () => ({
  onClose: vi.fn(),
  onChangeShortcut: vi.fn(async () => {}),
  onRestoreDefaults: vi.fn(async () => {}),
  onClearThumbCache: vi.fn(async () => {}),
  onClearCloudOpenCache: vi.fn(async () => {}),
  onClearStars: vi.fn(async () => {}),
  onClearBookmarks: vi.fn(async () => {}),
  onClearRecents: vi.fn(async () => {}),
})

describe('createSettingsModalViewModel filtering', () => {
  it('shows cloud thumbs row for cloud-thumb specific filter text', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    vm.filter.set('cloud thumbs')

    const model = vm.buildFilterModel({ ...DEFAULT_SETTINGS, cloudThumbs: true })
    expect(model.showThumbnails).toBe(true)
    expect(model.showCloudThumbsRow).toBe(true)
  })

  it('matches cloud thumbs row by network usage helper text', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    vm.filter.set('network usage')

    const model = vm.buildFilterModel({ ...DEFAULT_SETTINGS })
    expect(model.showCloudThumbsRow).toBe(true)
  })

  it('restoreDefaults resets cloudThumbs back to false', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    let next = { ...DEFAULT_SETTINGS, cloudThumbs: true }

    vm.restoreDefaults((value) => {
      next = value
    })

    expect(next.cloudThumbs).toBe(false)
  })

  it('restoreDefaults resets representative Linux-facing settings back to defaults', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    let next: Settings = {
      ...DEFAULT_SETTINGS,
      cloudEnabled: true,
      mountsPollMs: 1200,
      doubleClickMs: 550,
      logLevel: 'debug' as const,
      rclonePath: '/usr/local/bin/rclone',
      scrollbarWidth: 15,
    }

    vm.restoreDefaults((value) => {
      next = value
    })

    expect(next.cloudEnabled).toBe(DEFAULT_SETTINGS.cloudEnabled)
    expect(next.mountsPollMs).toBe(DEFAULT_SETTINGS.mountsPollMs)
    expect(next.doubleClickMs).toBe(DEFAULT_SETTINGS.doubleClickMs)
    expect(next.logLevel).toBe(DEFAULT_SETTINGS.logLevel)
    expect(next.rclonePath).toBe(DEFAULT_SETTINGS.rclonePath)
    expect(next.scrollbarWidth).toBe(DEFAULT_SETTINGS.scrollbarWidth)
  })

  it('restoreDefaults can preserve the current filter when requested', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    let next = { ...DEFAULT_SETTINGS, cloudEnabled: true }
    vm.filter.set('cloud')

    vm.restoreDefaults(
      (value) => {
        next = value
      },
      { clearFilter: false },
    )

    expect(next.cloudEnabled).toBe(false)
    expect(vm.buildFilterModel({ ...DEFAULT_SETTINGS }).showCloud).toBe(true)
  })

  it('shows the cloud section for cloud enable filter text', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    vm.filter.set('enable cloud')

    const model = vm.buildFilterModel({ ...DEFAULT_SETTINGS })
    expect(model.showCloud).toBe(true)
    expect(model.showCloudEnabledRow).toBe(true)
  })

  it('shows the performance section for mount refresh helper text', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    vm.filter.set('removable media')

    const model = vm.buildFilterModel({ ...DEFAULT_SETTINGS })
    expect(model.showPerformance).toBe(true)
    expect(model.showMountsPollRow).toBe(true)
  })
})
