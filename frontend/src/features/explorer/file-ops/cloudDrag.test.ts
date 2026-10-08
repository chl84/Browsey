import { describe, expect, it } from 'vitest'
import { cloudDragToken, isCloudDragOffer } from './cloudDrag'

describe('Browsey cloud references', () => {
  it('accepts one opaque reference without URI decoding or cloud path loss', () => {
    const token = 'a1'.repeat(32)
    expect(cloudDragToken([`browsey-drag://cloud/${token}`])).toBe(token)
    for (const paths of [[], ['rclone://Drive/file'], [`browsey-drag://cloud/${token}`, '/tmp/file'],
      [`browsey-drag://cloud/${token}?mode=cut`], [`browsey-drag://cloud/${token}#x`],
      [`browsey-drag://cloud/${token.toUpperCase()}`], ['browsey-drag://cloud/../file']]) {
      expect(cloudDragToken(paths)).toBeNull()
    }
  })
  it('recognizes malformed protocol offers so they cannot become local file drops', () => {
    expect(isCloudDragOffer(['browsey-drag://cloud/bad'])).toBe(true)
    expect(isCloudDragOffer(['/tmp/file'])).toBe(false)
  })
})
