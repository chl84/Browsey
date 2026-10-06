import assert from 'node:assert/strict'
import { test } from 'node:test'
import { releasedResources } from './resources.mjs'

function observer(states) {
  let attempt = -1, read = 0
  const ui = { handshake: async id => { assert.equal(id, 'owned-run'); attempt++; read = 0; return { cancelTasks: states[Math.min(attempt, states.length - 1)].tasks } },
    browser: { execute: async () => states[Math.min(attempt, states.length - 1)][read++ === 0 ? 'transfers' : 'searches'],
      waitUntil: async (predicate, options) => {
        assert.equal(options.timeout, 180_000)
        for (let count = 0; count < 4; count++) if (await predicate()) return
        throw new Error(options.timeoutMsg)
      } } }
  return { ui, attempts: () => attempt + 1 }
}
test('streamed results wait for backend and callback release using only bounded read-only observations', async () => {
  const o = observer([{ tasks: 1, transfers: [], searches: [42] }, { tasks: 0, transfers: [], searches: [42] }, { tasks: 0, transfers: [], searches: [] }])
  assert.deepEqual(await releasedResources(o.ui, 'owned-run'), { tasks: 0, transfers: [], searches: [] })
  assert.equal(o.attempts(), 3)
})
test('an unreleased task never becomes passing cleanup or causes an operation retry', async () => {
  const o = observer([{ tasks: 1, transfers: [], searches: [] }])
  await assert.rejects(releasedResources(o.ui, 'owned-run'), /did not finish and release/)
  assert.equal(o.attempts(), 4)
})
