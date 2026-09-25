import { describe, expect, it } from 'vitest'
import { fileDragPayload } from './fileDragPayload'

describe('file drag payload', () => {
  it('preserves every filename in one WebKit-safe envelope', () => {
    const paths = ['/tmp/first photo.jpg', '/tmp/æ #?%+\nsecond.jpg', '/tmp/a\\b']
    const payload = fileDragPayload(paths, true)!
    const url = new URL(payload)
    expect(url.protocol).toBe('browsey-drag:')
    expect(JSON.parse(url.searchParams.get('paths')!)).toEqual(paths)
    expect(payload).not.toMatch(/[\r\n]/)
  })

  it('encodes conventional file URIs outside the GTK bridge', () => {
    expect(fileDragPayload(['/tmp/a #?.txt', '/tmp/æ.txt'], false)).toBe('file:///tmp/a%20%23%3F.txt\r\nfile:///tmp/%C3%A6.txt\r\n')
    expect(fileDragPayload(['C:\\Photos\\a b.jpg', '\\\\server\\share\\a.jpg'], false)).toBe('file:///C:/Photos/a%20b.jpg\r\nfile://server/share/a.jpg\r\n')
  })

  it.each([true, false])('never exports cloud, mixed, empty or invalid selections (bridge=%s)', bridge => {
    for (const paths of [[], ['rclone://remote/file'], ['/tmp/a', 'rclone://remote/file'], ['relative'], ['/tmp/a\0b']]) {
      expect(fileDragPayload(paths, bridge)).toBeNull()
    }
  })
})
