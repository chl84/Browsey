import { describe, expect, it } from 'vitest'
import { iconPath } from './icons'

describe('3D model file icon', () => {
  it('appends the matching backend ID without shifting existing icons', () => {
    expect(iconPath(22)).toBe('icons/scalable/browsey/model_3d_file.svg')
    expect(iconPath(12)).toBe('icons/scalable/browsey/file.svg')
    expect(iconPath(21)).toBe('icons/scalable/browsey/cloud.svg')
  })

  it('preserves the existing fallback for unknown IDs', () => {
    expect(iconPath(999)).toBe(iconPath(0))
    expect(iconPath(undefined)).toBe(iconPath(0))
  })
})
