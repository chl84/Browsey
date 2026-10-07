import assert from 'node:assert/strict'
import {test} from 'node:test'
import {measuredSamples, feedbackLatencies, cancelFeedbackTimings} from './metrics.mjs'
test('measurements reject invalid/missing/unbounded evidence instead of reporting zero latency', () => {
  for (const values of [[], [-1], [NaN], [Infinity], new Array(1025).fill(1)]) assert.throws(() => measuredSamples(values, 'test'))
  assert.throws(() => feedbackLatencies([{time: 1, left: 12, top: 12, point: {x: 0, y: 0}}]))
})
test('Cancel measures actual feedback and terminal state before toast expiry, refusing untrusted/missing observations', () => {
  const samples = [{time: 9, cancelRequestedAt: null},
    {time: 10, cancelRequestedAt: 10, active: true, visible: true},
    {time: 12, cancelRequestedAt: 10, label: 'Cancelling…', active: true, visible: true},
    {time: 30, cancelRequestedAt: 10, active: false, visible: false, toast: 'Paste failed: Operation cancelled'},
    {time: 5030, cancelRequestedAt: 10, active: false, visible: false, toast: ''}]
  const result = cancelFeedbackTimings(samples)
  assert.deepEqual(result.acknowledgement.values, [2])
  assert.deepEqual(result.terminal.values, [20])
  assert.throws(() => cancelFeedbackTimings(samples.slice(0, 3)))
  assert.throws(() => cancelFeedbackTimings([{time: 10, cancelRequestedAt: null, active: false, visible: false, toast: 'Paste failed: cancelled'}]))
})
test('feedback timing uses the first matching frame per delivered event and rejects stale/negative pairs', () => {
  const frame = (time, event, left = 22) => ({time, left, top: 32, point: {time: event, x: 10, y: 20}})
  const result = feedbackLatencies([frame(9, 10), frame(11, 10, 0), frame(14, 10), frame(20, 10), frame(30, 25)])
  assert.deepEqual(result.values, [4, 5])
  assert.equal(result.count, 2)
  assert.equal(result.max, 5)
  assert.match(result.thresholds, /NONE/)
})
