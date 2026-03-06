import { describe, expect, it, vi } from 'vitest'

import { DEFAULT_SETTINGS } from '../settingsTypes'
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

  it('shows the cloud section for cloud enable filter text', () => {
    const vm = createSettingsModalViewModel(buildDeps())
    vm.filter.set('enable cloud')

    const model = vm.buildFilterModel({ ...DEFAULT_SETTINGS })
    expect(model.showCloud).toBe(true)
    expect(model.showCloudEnabledRow).toBe(true)
  })
})
