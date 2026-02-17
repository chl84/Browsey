import type { Partition } from './types'

export const iconPath = (file: string) => `/icons/scalable/${file}`

export const navIcon = (label: string) => {
  switch (label) {
    case 'Home':
      return iconPath('browsey/home.svg')
    case 'Recent':
      return iconPath('browsey/recent.svg')
    case 'Starred':
      return iconPath('browsey/starred.svg')
    case 'Network':
      return iconPath('browsey/network.svg')
    case 'Wastebasket':
      return iconPath('browsey/trash.svg')
    default:
      return iconPath('browsey/this_computer.svg')
  }
}

export const partitionIcon = (part: Partition) => {
  const fs = (part.fs ?? '').toLowerCase()
  if (fs === 'onedrive') {
    return iconPath('browsey/cloud.svg')
  }
  if (fs === 'sftp') {
    return iconPath('browsey/network.svg')
  }
  return part.removable ? iconPath('browsey/usb_disk.svg') : iconPath('browsey/disk.svg')
}

export const normalizePath = (p: string) => {
  if (!p) return ''
  const withSlashes = p.replace(/\\/g, '/')
  const trimmed = withSlashes.replace(/\/+$/, '')
  if (trimmed === '') return withSlashes.startsWith('/') ? '/' : ''
  if (/^[A-Za-z]:$/.test(trimmed)) return `${trimmed}/`
  return trimmed
}

export const isUnderMount = (path: string, mount: string) => {
  if (!path || !mount) return false
  const p = normalizePath(path)
  const m = normalizePath(mount)
  return p === m || p.startsWith(`${m}/`)
}

export const parentPath = (path: string) => {
  const normalized = normalizePath(path)
  if (!normalized || normalized === '/') return '/'
  const driveRoot = normalized.match(/^([A-Za-z]:)\/?$/)
  if (driveRoot) return `${driveRoot[1]}/`
  const drivePrefix = normalized.match(/^([A-Za-z]:)\//)
  const idx = normalized.lastIndexOf('/')
  if (idx <= 0) {
    return drivePrefix ? `${drivePrefix[1]}/` : '/'
  }
  if (drivePrefix && idx === drivePrefix[1].length) {
    return `${drivePrefix[1]}/`
  }
  return normalized.slice(0, idx)
}

export const formatSize = (size?: number | null) => {
  if (size === null || size === undefined) return ''
  if (size < 1024) return `${size} B`
  const units = ['kB', 'MB', 'GB', 'TB']
  let value = size / 1000
  let u = 0
  while (value >= 1000 && u < units.length - 1) {
    value /= 1000
    u++
  }
  return `${value.toFixed(1)} ${units[u]}`
}

export const formatItems = (count?: number | null) => {
  if (count === null || count === undefined) return ''
  const suffix = count === 1 ? 'item' : 'items'
  return `${count} ${suffix}`
}

export const formatSelectionLine = (count: number, noun: string, bytes?: number) => {
  if (count === 0) return ''
  const sizePart = bytes && bytes > 0 ? ` (${formatSize(bytes)})` : ''
  const suffix = count === 1 ? noun : `${noun}s`
  return `${count} ${suffix} selected${sizePart}`
}
