import { describe, expect, it } from 'vitest'
import { getErrorCode, getErrorMessage, normalizeError } from './error'

describe('shared error normalization', () => {
  it('preserves Error identity, stack, codes and details, including frozen errors', () => {
    const details = { operation: 'test' }
    const original = Object.freeze(Object.assign(new Error('Original message'), { code: 'typed_error', details }))
    expect(normalizeError(original)).toBe(original)
    expect(normalizeError(normalizeError(original))).toBe(original)
    expect(normalizeError(original).stack).toBe(original.stack)
    expect(normalizeError(original).details).toBe(details)
    expect(getErrorMessage(original)).toBe('Original message')
  })

  it.each([
    [{ message: 'Flat message' }, 'Flat message'],
    [{ error: 'Error string' }, 'Error string'],
    [{ cause: 'Cause string' }, 'Cause string'],
    [{ error: { message: 'Nested message' } }, 'Nested message'],
    [{ error: { error: 'Nested error' } }, 'Nested error'],
    [{ error: { cause: 'Nested cause' } }, 'Nested cause'],
    [{ message: '', error: 'Fallback error' }, 'Fallback error'],
    [{ message: '  ', error: { message: 'Nested fallback' } }, 'Nested fallback'],
    ['Plain diagnostic', 'Plain diagnostic'],
    ['{malformed json', '{malformed json'],
    ['', 'Unknown error'],
    ['  ', 'Unknown error'],
    [{}, 'Unknown error'],
    [undefined, 'Unknown error'],
    [null, 'null'],
    [false, 'false'],
    [42, '42'],
  ])('normalizes message shape %j', (value, expected) => {
    const error = normalizeError(value)
    expect(error).toBeInstanceOf(Error)
    expect(error.message).toBe(expected)
    expect(error.raw).toBe(value)
    expect(getErrorMessage(value)).toBe(expected)
  })

  it('retains typed metadata, preferring outer fields without losing explicit null details', () => {
    const raw = { code: ' outer_code ', message: 'Outer message', details: null,
      error: { code: 'inner_code', message: 'Inner message', details: { inner: true } } }
    expect(normalizeError(raw)).toMatchObject({ code: 'outer_code', message: 'Outer message', details: null, raw })
    const nested = { error: { code: ' nested_code ', message: 'Nested message', details: { count: 2 } } }
    expect(normalizeError(nested)).toMatchObject({ code: 'nested_code', message: 'Nested message', details: { count: 2 }, raw: nested })
  })

  it('extracts JSON-encoded IPC metadata while preserving the original string message', () => {
    const raw = JSON.stringify({ code: 'encoded_code', message: 'JSON message', details: { count: 2 } })
    expect(normalizeError(raw)).toMatchObject({ code: 'encoded_code', message: raw, details: { count: 2 }, raw })
  })

  it('handles empty Error messages and circular objects without throwing', () => {
    const error = Object.freeze(new Error(''))
    expect(normalizeError(error)).toBe(error)
    expect(getErrorMessage(error)).toBe('Unknown error')
    const circular: { error?: unknown } = {}
    circular.error = circular
    expect(getErrorMessage(circular)).toBe('Unknown error')
    expect(normalizeError(circular).raw).toBe(circular)
  })

  it.each([
    [{ code: ' direct_code ' }, 'direct_code'],
    [{ code: '', error: { code: 'nested_code' } }, 'nested_code'],
    [JSON.stringify({ code: 'encoded_code' }), 'encoded_code'],
    [{ error: { code: 'nested_code' } }, 'nested_code'],
    [Object.freeze(Object.assign(new Error('Typed error'), { code: ' frozen_code ' })), 'frozen_code'],
    [Object.assign(new Error('Legacy wrapper'), { raw: { error: { code: 'nested_code' } } }), 'nested_code'],
    [Object.assign(new Error('Legacy JSON wrapper'), { raw: JSON.stringify({ code: 'encoded_code' }) }), 'encoded_code'],
    ['permission_denied', null],
    [new Error('permission_denied'), null],
    [{ code: 13, message: 'permission_denied' }, null],
    ['{malformed', null],
    [null, null],
  ])('reads typed codes without prose inference for %j', (value, code) => {
    expect(getErrorCode(value)).toBe(code)
    expect(getErrorCode(normalizeError(value))).toBe(code)
  })

  it('bounds raw fallback and retains immutable Error identity', () => {
    const original = Object.freeze(Object.assign(new Error('Message'), { code: ' typed_code ' }))
    expect(getErrorCode(original)).toBe('typed_code')
    expect(original.code).toBe(' typed_code ')
    expect(normalizeError(original)).toBe(original)
    const cycle: { raw?: unknown } = {}
    cycle.raw = cycle
    expect(getErrorCode(cycle)).toBeNull()
  })
})
