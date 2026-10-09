import { describe, expect, it } from 'vitest'
import { iconPath } from './icons'

describe('Explorer icon IDs', () => {
  it('appends the matching backend ID without shifting existing icons', () => {
    expect(iconPath(22)).toBe('icons/scalable/browsey/model_3d_file.svg')
    expect(iconPath(23)).toBe('icons/scalable/browsey/network_folder.svg')
    expect(iconPath(24)).toBe('icons/scalable/browsey/document_file.svg')
    expect(iconPath(25)).toBe('icons/scalable/browsey/code_file.svg')
    expect(iconPath(26)).toBe('icons/scalable/browsey/package_file.svg')
    expect(iconPath(27)).toBe('icons/scalable/browsey/disk_image_file.svg')
    expect(iconPath(28)).toBe('icons/scalable/browsey/font_file.svg')
    expect(iconPath(29)).toBe('icons/scalable/browsey/ebook_file.svg')
    expect(iconPath(10)).toBe('icons/scalable/browsey/folder.svg')
    expect(iconPath(12)).toBe('icons/scalable/browsey/file.svg')
    expect(iconPath(21)).toBe('icons/scalable/browsey/cloud.svg')
  })

  it('preserves the existing fallback for unknown IDs', () => {
    expect(iconPath(999)).toBe(iconPath(0))
    expect(iconPath(undefined)).toBe(iconPath(0))
  })
})
