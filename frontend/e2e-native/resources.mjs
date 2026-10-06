/* global window */
import assert from 'node:assert/strict'
import { transferListeners } from './cancellation.mjs'

export async function searchListeners(ui) {
  return ui.browser.execute(() => {
    const listeners = window.__internal_unstable_listeners_object_id__, callbacks = window.__TAURI_INTERNALS__?.callbacks
    if (!listeners || typeof callbacks?.has !== 'function') throw Error('Native callback registry unavailable')
    return Object.getOwnPropertyNames(listeners).filter(name => /^search-progress-/.test(name))
      .flatMap(name => Object.getOwnPropertyNames(listeners[name]).map(id => listeners[name][id].handlerId).filter(id => callbacks.has(id)))
  })
}
// Streamed matches are not a completion acknowledgement. Observe actual task
// and callback release without replaying search, navigation or a mutation.
export async function releasedResources(ui, runId) {
  let state
  await ui.browser.waitUntil(async () => {
    state = { tasks: (await ui.handshake(runId)).cancelTasks, transfers: await transferListeners(ui), searches: await searchListeners(ui) }
    return state.tasks === 0 && state.transfers.length === 0 && state.searches.length === 0
  }, { timeout: 180_000, interval: 150, timeoutMsg: 'Native tasks/search/transfer callbacks did not finish and release' })
  assert.deepEqual(state, { tasks: 0, transfers: [], searches: [] })
  return state
}
