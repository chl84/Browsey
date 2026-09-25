import { writable } from 'svelte/store'
import {
  loadSystemTheme,
  loadThemeMode,
  storeThemeMode,
} from '../services/settings.service'
import type { SystemTheme, ThemeMode } from './types'

const POLL_INTERVAL_MS = 2_000

const paletteVariables = [
  '--bg',
  '--bg-alt',
  '--bg-raised',
  '--bg-button',
  '--bg-hover',
  '--border',
  '--border-strong',
  '--border-accent',
  '--border-accent-strong',
  '--accent-primary',
  '--accent-danger',
  '--accent-warning',
  '--accent-error-text',
  '--danger',
  '--panel',
  '--border-subtle',
  '--selection-border',
  '--selection-fill',
  '--lasso-fill',
  '--selection-shadow',
  '--drop-allowed-bg',
  '--drop-allowed-border',
  '--drop-allowed-shadow',
  '--drop-blocked-bg',
  '--drop-blocked-border',
  '--drop-blocked-shadow',
  '--drag-ghost-bg',
  '--drag-ghost-dot-bg',
  '--fg',
  '--fg-strong',
  '--fg-muted',
  '--fg-dim',
  '--fg-pill',
  '--win-btn-bg',
  '--win-btn-fg',
  '--win-btn-border',
  '--win-btn-hover-bg',
  '--win-btn-close-hover-bg',
  '--win-btn-close-hover-border',
  '--btn-primary-bg',
  '--btn-danger-bg',
  '--btn-danger-border',
  '--btn-danger-fg',
  '--pill-error-border',
  '--pill-error-fg',
  '--tooltip-bg',
  '--tooltip-fg',
  '--tooltip-border',
  '--focus-ring-color',
] as const

export const themeMode = writable<ThemeMode>('dark')
export const activeSystemTheme = writable<SystemTheme | null>(null)

let currentMode: ThemeMode = 'dark'
let currentPalette: SystemTheme | null = null
let highContrast = false
let initialized = false
let pollTimer: ReturnType<typeof setInterval> | null = null
let systemColorScheme: MediaQueryList | null = null
let colorSchemeListener: (() => void) | null = null

const colorWithAlpha = (hex: string, alpha: number) => {
  const value = hex.slice(1)
  const red = Number.parseInt(value.slice(0, 2), 16)
  const green = Number.parseInt(value.slice(2, 4), 16)
  const blue = Number.parseInt(value.slice(4, 6), 16)
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`
}

const resolvedMode = (): 'light' | 'dark' => {
  if (currentMode !== 'system') return currentMode
  return currentPalette?.mode ?? (systemColorScheme?.matches ? 'dark' : 'light')
}

const clearPalette = () => {
  const root = document.documentElement
  paletteVariables.forEach((variable) => root.style.removeProperty(variable))
  root.removeAttribute('data-system-theme')
}

const setPalette = (theme: SystemTheme) => {
  const root = document.documentElement
  const set = (variable: string, value: string) => root.style.setProperty(variable, value)
  const selectionBorder = colorWithAlpha(theme.accent, 0.72)
  const selectionFill = colorWithAlpha(theme.selection, 0.82)

  set('--bg', theme.darkerBackground)
  set('--bg-alt', theme.darkBackground)
  set('--bg-raised', theme.background)
  set('--bg-button', theme.darkBackground)
  set('--bg-hover', theme.lighterBackground)
  set('--border', theme.muted)
  set('--border-strong', theme.background)
  set('--border-accent', theme.lightForeground)
  set('--border-accent-strong', theme.foreground)
  set('--accent-primary', theme.accent)
  set('--accent-danger', theme.red)
  set('--accent-warning', theme.yellow)
  set('--accent-error-text', theme.red)
  set('--danger', theme.red)
  set('--panel', theme.darkBackground)
  set('--border-subtle', theme.muted)
  set('--selection-border', selectionBorder)
  set('--selection-fill', selectionFill)
  // Keep the drag rectangle translucent without weakening selected file highlights.
  set('--lasso-fill', colorWithAlpha(theme.selection, 0.16))
  set('--selection-shadow', `0 0 0 1px ${colorWithAlpha(theme.accent, 0.28)}`)
  set('--drop-allowed-bg', colorWithAlpha(theme.accent, 0.14))
  set('--drop-allowed-border', colorWithAlpha(theme.accent, 0.72))
  set('--drop-allowed-shadow', `inset 0 0 0 1px ${colorWithAlpha(theme.accent, 0.42)}`)
  set('--drop-blocked-bg', colorWithAlpha(theme.red, 0.16))
  set('--drop-blocked-border', colorWithAlpha(theme.red, 0.72))
  set('--drop-blocked-shadow', `inset 0 0 0 1px ${colorWithAlpha(theme.red, 0.4)}`)
  set('--drag-ghost-bg', `linear-gradient(120deg, ${theme.accent}, ${theme.blue})`)
  set('--drag-ghost-dot-bg', theme.foreground)
  set('--fg', theme.foreground)
  set('--fg-strong', theme.brightForeground)
  set('--fg-muted', theme.lightForeground)
  set('--fg-dim', theme.darkForeground)
  set('--fg-pill', theme.foreground)
  set('--win-btn-bg', theme.darkBackground)
  set('--win-btn-fg', theme.foreground)
  set('--win-btn-border', theme.muted)
  set('--win-btn-hover-bg', theme.lighterBackground)
  set('--win-btn-close-hover-bg', theme.red)
  set('--win-btn-close-hover-border', theme.red)
  set('--btn-primary-bg', theme.accent)
  set('--btn-danger-bg', colorWithAlpha(theme.red, 0.16))
  set('--btn-danger-border', colorWithAlpha(theme.red, 0.65))
  set('--btn-danger-fg', theme.red)
  set('--pill-error-border', theme.red)
  set('--pill-error-fg', theme.red)
  set('--tooltip-bg', theme.background)
  set('--tooltip-fg', theme.foreground)
  set('--tooltip-border', theme.muted)
  set('--focus-ring-color', colorWithAlpha(theme.accent, 0.58))
  root.dataset.systemTheme = theme.name
}

const applyCurrentTheme = () => {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const scheme = resolvedMode()
  root.dataset.theme = scheme
  root.dataset.themeSource = currentMode
  root.style.colorScheme = scheme
  if (currentMode === 'system' && currentPalette && !highContrast) {
    setPalette(currentPalette)
  } else {
    clearPalette()
  }
}

const refreshSystemTheme = async () => {
  if (currentMode !== 'system') return
  try {
    const nextPalette = await loadSystemTheme()
    if (JSON.stringify(nextPalette) !== JSON.stringify(currentPalette)) {
      currentPalette = nextPalette
      activeSystemTheme.set(nextPalette)
      applyCurrentTheme()
    }
  } catch (error) {
    console.warn('Failed to read system theme', error)
    if (currentPalette) {
      currentPalette = null
      activeSystemTheme.set(null)
      applyCurrentTheme()
    }
  }
}

const syncPolling = () => {
  if (currentMode === 'system' && !pollTimer) {
    pollTimer = setInterval(() => void refreshSystemTheme(), POLL_INTERVAL_MS)
  } else if (currentMode !== 'system' && pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

export const initializeThemeController = async () => {
  if (initialized || typeof window === 'undefined') return
  initialized = true
  systemColorScheme = window.matchMedia('(prefers-color-scheme: dark)')
  colorSchemeListener = () => {
    if (currentMode === 'system' && !currentPalette) applyCurrentTheme()
  }
  systemColorScheme.addEventListener('change', colorSchemeListener)

  try {
    const saved = await loadThemeMode()
    if (saved) {
      currentMode = saved
    } else {
      currentMode = localStorage.getItem('browsey-theme') === 'light' ? 'light' : 'dark'
      await storeThemeMode(currentMode)
    }
  } catch (error) {
    console.warn('Failed to load theme preference', error)
  }

  themeMode.set(currentMode)
  await refreshSystemTheme()
  applyCurrentTheme()
  syncPolling()
}

export const setThemeMode = async (nextMode: ThemeMode) => {
  currentMode = nextMode
  themeMode.set(nextMode)
  if (nextMode === 'system') {
    await refreshSystemTheme()
  } else {
    currentPalette = null
    activeSystemTheme.set(null)
  }
  applyCurrentTheme()
  syncPolling()
  try {
    await storeThemeMode(nextMode)
  } catch (error) {
    console.warn('Failed to store theme preference', error)
  }
}

export const setThemeHighContrast = (enabled: boolean) => {
  highContrast = enabled
  applyCurrentTheme()
}

export const destroyThemeController = () => {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
  if (systemColorScheme && colorSchemeListener) {
    systemColorScheme.removeEventListener('change', colorSchemeListener)
  }
  systemColorScheme = null
  colorSchemeListener = null
  initialized = false
}
