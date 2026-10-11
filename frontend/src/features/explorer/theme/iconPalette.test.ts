import { describe, expect, it } from 'vitest'
import { composite, contrast, createIconPalette, type IconColors } from './iconPalette'

const nord: IconColors = {
  accent: '#81a1c1', foreground: '#d8dee9', red: '#bf616a', yellow: '#ebcb8b',
  orange: '#d08770', green: '#a3be8c', blue: '#81a1c1', magenta: '#b48ead',
}
const ristretto: IconColors = {
  accent: '#f38d70', foreground: '#e6d9db', red: '#fd6883', yellow: '#f9cc6c',
  orange: '#fb9a77', green: '#adda78', blue: '#f38d70', magenta: '#a8a9eb',
}
const light: IconColors = { ...nord, foreground: '#1f232b' }
const browseyLight: IconColors = {
  accent: '#1e88e5', foreground: '#1f232b', red: '#d04444', yellow: '#dba92d',
  orange: '#bc8051', green: '#63976a', blue: '#6c93bd', magenta: '#9973b4',
}
const cases = [
  { name: 'dark', colors: nord, surfaces: ['#101010', '#0d0d0d', '#181818', '#1f1f1f'] },
  { name: 'light', colors: light, surfaces: ['#f7f7f8', '#f0f1f3', '#e7e9ed', '#ffffff'] },
  { name: 'Browsey light', colors: browseyLight, surfaces: ['#f7f7f8', '#f0f1f3', '#e7e9ed', '#ffffff'] },
  { name: 'Nord', colors: nord, surfaces: ['#191c23', '#222730', '#3b4252', '#2e3440'] },
  { name: 'Ristretto', colors: ristretto, surfaces: ['#181414', '#211b1b', '#3d2f2a', '#2c2525'] },
  { name: 'light selection', colors: light, surfaces: [composite('rgba(30, 136, 229, 0.14)', '#f7f7f8')] },
  { name: 'dark selection', colors: nord, surfaces: ['#434c5e'] },
  { name: 'bright selection on a dark theme', colors: ristretto, surfaces: ['#d8dee9'] },
]

describe('icon contrast', () => {
  it('matches known contrast ratios and composites translucent selection fills', () => {
    expect(contrast('#000000', '#ffffff')).toBe(21)
    expect(contrast('#808080', '#808080')).toBe(1)
    expect(composite('rgba(255, 255, 255, 0.5)', '#000000')).toBe('#808080')
  })

  it.each(cases)('$name keeps silhouettes, document details and small labels readable', ({ colors, surfaces }) => {
    const palette = createIconPalette(colors, surfaces)
    for (const surface of surfaces) {
      for (const role of ['folder', 'folder-tab', 'home', 'home-shade', 'bookmark', 'bookmark-shade', 'neutral', 'neutral-link', 'disk', 'disk-tab', 'yellow', 'foreground'] as const) {
        expect(contrast(palette[role], surface), `${role} against ${surface}`).toBeGreaterThanOrEqual(3)
      }
      // Light documents use a dark outline; dark documents use their light body.
      expect(Math.max(contrast(palette.paper, surface), contrast(palette.ink, surface))).toBeGreaterThanOrEqual(3)
    }
    for (const role of ['paper', 'fold', 'blue', 'blue-mid', 'blue-strong', 'green', 'green-mid', 'green-strong', 'purple', 'purple-mid', 'orange', 'orange-mid', 'orange-strong'] as const) {
      expect(contrast(palette.ink, palette[role]), `document ink against ${role}`).toBeGreaterThanOrEqual(4.5)
    }
    for (const [foreground, background] of [['folder-detail', 'folder'], ['folder-detail', 'folder-inset'], ['on-red', 'red'], ['on-yellow', 'yellow'], ['neutral-detail', 'neutral'], ['disk-detail', 'disk']] as const) {
      expect(contrast(palette[foreground], palette[background]), `${foreground} against ${background}`).toBeGreaterThanOrEqual(4.5)
    }
  })

  it.each(cases)('$name strengthens contrast without removing its palette in high contrast', ({ colors, surfaces }) => {
    const palette = createIconPalette(colors, surfaces, true)
    expect(contrast(palette['folder-detail'], palette['folder-inset'])).toBeGreaterThanOrEqual(4.5)
    for (const surface of surfaces) {
      expect(contrast(palette.folder, surface)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(palette.foreground, surface)).toBeGreaterThanOrEqual(4.5)
    }
  })

  it('retains the warm accent of the active Ristretto theme instead of fixed blue folders', () => {
    const palette = createIconPalette(ristretto, ['#181414', '#3d2f2a'])
    expect(palette.folder).toBe(ristretto.accent)
    expect(palette.folder).not.toBe(createIconPalette(nord, ['#191c23']).folder)
  })
})
