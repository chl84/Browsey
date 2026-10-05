import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Key } from 'webdriverio'
import { NativeUi } from './ui.mjs'

test('native modifier actions use WebDriver key codes and release on failure', async () => {
  const values = []
  let released = 0
  const action = { down(value) { values.push(['down', value]); return this },
    up(value) { values.push(['up', value]); return this },
    async perform() { throw new Error('synthetic delivery failure') } }
  const ui = new NativeUi({ action: () => action, releaseActions: async () => { released++ } }, [])
  await assert.rejects(ui.chord('Delete', 'Shift'), /delivery failure/)
  assert.deepEqual(values, [['down', Key.Shift], ['down', Key.Delete], ['up', Key.Delete], ['up', Key.Shift]])
  assert.equal(released, 1)
})

test('input mismatch or lost focus aborts before the caller can submit', async () => {
  const action = { down() { return this }, up() { return this }, perform: async () => {} }
  const ui = new NativeUi({ releaseActions: async () => {}, action: () => action, keys: async () => {} }, [])
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => true,
    setValue: async () => {}, getValue: async () => 'wrong/value' }, '/owned/input_æøå'), /intended value/)
  await assert.rejects(ui.fill({ click: async () => {}, isFocused: async () => false }, '/owned/input_æøå'), /have focus/)
})

test('an app-reported failure stops readiness even when the operation flag is idle', async () => {
  const ui = new NativeUi({ execute: async () => ({ idle: true, errors: ['Paste failed: Cloud operation failed.'] }),
    waitUntil: async predicate => assert.equal(await predicate(), true) }, [])
  await assert.rejects(ui.idle(), error => error.failureKind === 'APP_REPORTED_ERROR' && /Paste failed/.test(error.message))
})

test('a cut must be acknowledged before navigation/paste, without retrying the shortcut', async () => {
  const calls = []
  const ui = new NativeUi({ execute: async () => undefined,
    waitUntil: async predicate => assert.equal(await predicate(), true) }, ['/owned/files'])
  ui.navigate = async path => { calls.push(['navigate', path]) }
  ui.select = async () => {}
  ui.chord = async key => { calls.push(['key', key]) }
  ui.idle = async expected => {
    assert.deepEqual(expected, { toast: 'Cut', cutPath: '/owned/files/source/sample.txt' })
    throw new Error('Cut acknowledgement absent')
  }
  await assert.rejects(ui.transfer('/owned/files/source/sample.txt', '/owned/files/dest', true), /acknowledgement absent/)
  assert.deepEqual(calls, [['navigate', '/owned/files/source'], ['key', 'x']])
})
