import { afterEach, expect, it, vi } from 'vitest'
import { copyTextToSystemClipboard } from './clipboard'

const originalCommand = Object.getOwnPropertyDescriptor(document, 'execCommand')
afterEach(() => {
  vi.unstubAllGlobals()
  if (originalCommand) Object.defineProperty(document, 'execCommand', originalCommand)
  else Reflect.deleteProperty(document, 'execCommand')
  document.querySelectorAll('textarea').forEach(node => node.remove())
})

it('removes its fallback field and reports a clipboard command exception', async () => {
  vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(new Error('API unavailable')) } })
  const command = vi.fn(() => { throw new Error('Clipboard command unavailable') })
  Object.defineProperty(document, 'execCommand', { configurable: true, value: command })
  expect(await copyTextToSystemClipboard('https://example.invalid/demo')).toEqual({ ok: false, error: 'Clipboard command unavailable' })
  expect(command).toHaveBeenCalledExactlyOnceWith('copy')
  expect(document.querySelector('textarea')).toBeNull()
})

it('uses the modern clipboard without creating a temporary field', async () => {
  const writeText = vi.fn().mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { clipboard: { writeText } })
  expect(await copyTextToSystemClipboard('demo')).toEqual({ ok: true })
  expect(writeText).toHaveBeenCalledExactlyOnceWith('demo')
  expect(document.querySelector('textarea')).toBeNull()
})
