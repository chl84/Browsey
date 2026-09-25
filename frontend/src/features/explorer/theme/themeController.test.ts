import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SystemTheme } from './types'
import {
  destroyThemeController,
  setThemeHighContrast,
  setThemeMode,
} from './themeController'

const { loadSystemTheme } = vi.hoisted(() => ({ loadSystemTheme: vi.fn() }))
vi.mock('../services/settings.service', () => ({
  loadSystemTheme,
  loadThemeMode: vi.fn(),
  storeThemeMode: vi.fn().mockResolvedValue(undefined),
}))

const palette: SystemTheme = {
  name: 'Test', mode: 'dark', accent: '#81a1c1', selection: '#434c5e',
  muted: '#4c566a', background: '#2e3440', darkBackground: '#222730',
  darkerBackground: '#191c23', lighterBackground: '#3b4252',
  foreground: '#d8dee9', darkForeground: '#667080', lightForeground: '#adb5c4',
  brightForeground: '#ffffff', red: '#bf616a', yellow: '#ebcb8b', orange: '#d08770',
  green: '#a3be8c', cyan: '#88c0d0', blue: '#81a1c1', magenta: '#b48ead',
}

const value = (name: string) => document.documentElement.style.getPropertyValue(name)

beforeEach(async () => {
  await setThemeMode('dark')
  setThemeHighContrast(false)
  loadSystemTheme.mockResolvedValue(palette)
})

afterEach(async () => {
  await setThemeMode('dark')
  setThemeHighContrast(false)
  destroyThemeController()
  vi.clearAllMocks()
})

describe('lasso fill', () => {
  it('uses a translucent system-theme fill without changing selected file highlights', async () => {
    await setThemeMode('system')
    expect(value('--lasso-fill')).toBe('rgba(67, 76, 94, 0.16)')
    expect(value('--selection-fill')).toBe('rgba(67, 76, 94, 0.82)')
  })

  it.each(['light', 'dark'] as const)('restores the CSS fallback when switching to %s', async (mode) => {
    await setThemeMode('system')
    await setThemeMode(mode)
    expect(value('--lasso-fill')).toBe('')
    expect(value('--selection-fill')).toBe('')
  })

  it('uses the high-contrast fallback and restores the system fill when disabled', async () => {
    await setThemeMode('system')
    setThemeHighContrast(true)
    expect(value('--lasso-fill')).toBe('')
    setThemeHighContrast(false)
    expect(value('--lasso-fill')).toBe('rgba(67, 76, 94, 0.16)')
  })

  it('clears the system fill when the system palette becomes unavailable', async () => {
    await setThemeMode('system')
    loadSystemTheme.mockResolvedValue(null)
    await setThemeMode('system')
    expect(value('--lasso-fill')).toBe('')
    expect(value('--selection-fill')).toBe('')
  })
})
