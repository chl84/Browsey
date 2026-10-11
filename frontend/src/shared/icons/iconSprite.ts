// Only bundled Browsey assets enter the sprite; file thumbnails are never SVG markup.
const sources = import.meta.glob<string>('../../../../resources/icons/scalable/*.svg', {
  query: '?raw', import: 'default', eager: true,
})

const symbols = new Map<string, string>()

// Canonical SVGs use literal colors so native image previewers can render them.
// Palette annotations supply the browser theme without maintaining another set.
const themePaint = (source: string) => source.replace(/<[a-z][^>]*>/gi, element => {
  for (const [, paint, role] of element.matchAll(/\sdata-icon-(fill|stroke|stop-color)="([a-z-]+)"/g)) {
    const attribute = new RegExp(`(\\s${paint}=)"([^"]+)"`)
    if (!attribute.test(element)) throw new Error(`Missing ${paint} for icon color ${role}`)
    element = element.replace(attribute, (_, prefix: string, color: string) => `${prefix}"var(--icon-${role}, ${color})"`)
      .replace(` data-icon-${paint}="${role}"`, '')
  }
  return element
})

const sprite = Object.entries(sources).map(([path, source]) => {
  const filename = path.slice(path.lastIndexOf('/') + 1)
  const category = ['eye-svgrepo-com.svg', 'padlock.svg'].includes(filename) ? 'status' : 'browsey'
  const name = `${category}/${filename}`
  const id = `browsey-icon-${name.replace(/[^a-z0-9_-]/gi, '-')}`
  symbols.set(name, id)
  const svg = themePaint(source).match(/^\s*<svg\b([^>]*)>([\s\S]*)<\/svg>\s*$/)
  if (!svg) throw new Error(`Invalid bundled icon: ${name}`)
  const viewBox = svg[1].match(/\bviewBox="([^"]+)"/)?.[1]
  if (!viewBox) throw new Error(`Missing bundled icon viewBox: ${name}`)
  // Keep root presentation attributes on a group, including status-icon strokes.
  const attributes = svg[1].replace(/\s+(?:xmlns|width|height|viewBox|role|aria-label)="[^"]*"/g, '')
  const body = svg[2].replace(/\bid="([^"]+)"/g, (_, local: string) => `id="${id}-${local}"`)
    .replace(/url\(#([^)]+)\)/g, (_, local: string) => `url(#${id}-${local})`)
  return `<symbol id="${id}" viewBox="${viewBox}"><g${attributes}>${body}</g></symbol>`
}).join('')

export const iconSprite = sprite
export const iconNames = [...symbols.keys()]
export const iconSymbol = (src: string) => symbols.get(src.replace(/^\/?icons\/scalable\//, ''))
  ?? symbols.get('browsey/shortcut.svg')!
