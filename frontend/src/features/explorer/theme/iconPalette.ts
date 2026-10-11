import type { SystemTheme } from './types'

type RGB = [number, number, number]
export type IconColors = Pick<SystemTheme, 'accent' | 'foreground' | 'red' | 'yellow' | 'orange' | 'green' | 'blue' | 'magenta'>

export const rgb = (color: string): RGB => {
  const hex = color.match(/^#([a-f0-9]{6})$/i)?.[1]
  if (hex) return [0, 2, 4].map(offset => parseInt(hex.slice(offset, offset + 2), 16)) as RGB
  const values = color.match(/^rgba?\(([^)]+)\)$/)?.[1].split(',').map(Number)
  if (values && values.length >= 3 && values.every(Number.isFinite)) return values.slice(0, 3) as RGB
  throw new Error(`Unsupported icon palette color: ${color}`)
}

const hex = (color: RGB) => `#${color.map(value => Math.round(value).toString(16).padStart(2, '0')).join('')}`

export const mix = (base: string, tint: string, amount: number) => {
  const a = rgb(base), b = rgb(tint)
  return hex(a.map((value, i) => value + (b[i] - value) * amount) as RGB)
}

export const composite = (color: string, background: string) => {
  const alpha = color.match(/^rgba\([^)]+,\s*([\d.]+)\)$/)?.[1]
  return mix(background, hex(rgb(color)), alpha === undefined ? 1 : Number(alpha))
}

const luminance = (color: string) => {
  const linear = rgb(color).map(value => {
    const channel = value / 255
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  })
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}

export const contrast = (a: string, b: string) => {
  const x = luminance(a), y = luminance(b)
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
}

// Adjust only as far as necessary toward black/white, retaining palette hue.
const readable = (color: string, surfaces: string[], minimum = 3.2) => {
  const passes = (candidate: string) => surfaces.every(surface => contrast(candidate, surface) >= minimum)
  if (passes(color)) return color
  for (let step = 1; step <= 100; step++) {
    const darker = mix(color, '#000000', step / 100)
    const lighter = mix(color, '#ffffff', step / 100)
    if (passes(darker)) return darker
    if (passes(lighter)) return lighter
  }
  // Conflicting surface luminances cannot always share one color. Selection
  // gets a separate palette; this last resort still maximizes worst contrast.
  return ['#000000', '#ffffff'].sort((a, b) =>
    Math.min(...surfaces.map(surface => contrast(b, surface))) - Math.min(...surfaces.map(surface => contrast(a, surface))),
  )[0]
}

export const createIconPalette = (colors: IconColors, surfaces: string[], highContrast = false) => {
  const minimum = highContrast ? 4.5 : 3.2
  const light = luminance(surfaces[0]) > 0.35
  const paper = light ? mix(surfaces[0], colors.foreground, 0.05)
    : readable(mix(surfaces[0], colors.foreground, 0.84), surfaces, minimum)
  const pane = (color: string, amount = 0.24) => mix(paper, color, amount)
  const blue = pane(colors.blue), green = pane(colors.green), purple = pane(colors.magenta), orange = pane(colors.orange)
  const blueMid = pane(colors.blue, 0.42), greenMid = pane(colors.green, 0.42)
  const purpleMid = pane(colors.magenta, 0.42), orangeMid = pane(colors.orange, 0.42)
  const blueStrong = pane(colors.blue, 0.58), greenStrong = pane(colors.green, 0.58), orangeStrong = pane(colors.orange, 0.58)
  const fold = mix(paper, light ? colors.foreground : surfaces[0], 0.12)
  const ink = readable(light ? colors.foreground : surfaces[0],
    [paper, fold, blue, green, purple, orange, blueMid, greenMid, purpleMid, orangeMid, blueStrong, greenStrong, orangeStrong, ...(light ? surfaces : [])], 4.5)
  const body = (color: string) => readable(color, surfaces, minimum)
  const shade = (color: string) => body(mix(color, light ? '#000000' : '#ffffff', 0.16))
  const on = (color: string) => readable(light ? '#ffffff' : surfaces[0], [color], 4.5)
  const folder = body(colors.accent), home = body(colors.green), bookmark = body(colors.orange)
  const folderDetail = on(folder)
  // Shared glyphs place meaningful strokes on both the body and inset surfaces.
  const folderInset = readable(mix(folder, folderDetail, 0.18), [folderDetail], 4.5)
  const neutral = body(mix(surfaces[0], colors.foreground, 0.78)), disk = body(colors.magenta)
  const red = readable(colors.red, [paper], minimum), yellow = body(colors.yellow)
  return {
    paper, fold, ink,
    folder, 'folder-tab': shade(folder), 'folder-detail': folderDetail,
    'folder-inset': folderInset, 'folder-edge': mix(folder, folderDetail, 0.35),
    blue, 'blue-mid': blueMid, 'blue-strong': blueStrong,
    green, 'green-mid': greenMid, 'green-strong': greenStrong,
    purple, 'purple-mid': purpleMid, orange, 'orange-mid': orangeMid, 'orange-strong': orangeStrong,
    red, 'on-red': on(red), yellow, 'on-yellow': on(yellow),
    home, 'home-shade': shade(home), bookmark, 'bookmark-shade': shade(bookmark), 'bookmark-edge': on(bookmark),
    neutral, 'neutral-detail': on(neutral), 'neutral-link': body(mix(surfaces[0], colors.foreground, 0.5)),
    disk, 'disk-tab': shade(disk), 'disk-detail': on(disk), 'disk-inset': mix(disk, on(disk), 0.2),
    foreground: readable(colors.foreground, surfaces, 4.5),
  }
}

export const applyIconPalette = (mode: 'light' | 'dark', theme: SystemTheme | null, highContrast: boolean) => {
  const root = document.documentElement
  const style = getComputedStyle(root)
  const defaults = mode === 'light'
    ? { bg: '#f7f7f8', alt: '#f0f1f3', hover: '#e7e9ed', raised: '#ffffff', fg: '#1f232b', selection: 'rgba(30, 136, 229, 0.14)' }
    : { bg: '#101010', alt: '#0d0d0d', hover: '#181818', raised: '#1f1f1f', fg: '#e8e2d9', selection: 'rgba(160, 160, 160, 0.16)' }
  const value = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback
  const surfaces = [value('--bg', defaults.bg), value('--bg-alt', defaults.alt), value('--bg-hover', defaults.hover), value('--bg-raised', defaults.raised)]
  const colors: IconColors = theme ?? {
    accent: value('--accent-primary', '#1e88e5'), foreground: value('--fg', defaults.fg),
    red: value('--accent-danger', '#d04444'), yellow: value('--accent-warning', '#dba92d'),
    orange: '#bc8051', green: '#63976a', blue: '#6c93bd', magenta: '#9973b4',
  }
  for (const property of ['--drop-allowed-bg', '--drop-blocked-bg']) {
    const fill = style.getPropertyValue(property).trim()
    if (fill) surfaces.push(composite(fill, surfaces[0]))
  }
  const selected = composite(value('--selection-fill', defaults.selection), surfaces[0])
  const normalPalette = createIconPalette(colors, surfaces, highContrast)
  const selectedPalette = createIconPalette(colors, [selected], highContrast)
  for (const [name, color] of Object.entries(normalPalette)) root.style.setProperty(`--icon-${name}`, color)
  for (const [name, color] of Object.entries(selectedPalette)) root.style.setProperty(`--icon-selected-${name}`, color)
}
