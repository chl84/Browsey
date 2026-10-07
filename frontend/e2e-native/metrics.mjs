import assert from 'node:assert/strict'

// Descriptive values only. Callers must state clocks, cache and boundaries.
export function measuredSamples(values, context) {
  assert.ok(Array.isArray(values) && values.length > 0 && values.length <= 1024)
  assert.ok(values.every(value => Number.isFinite(value) && value >= 0), 'Invalid timing sample')
  const sorted = [...values].sort((a, b) => a - b)
  const quantile = q => sorted[Math.max(0, Math.ceil(q * sorted.length) - 1)]
  return {unit: 'ms', count: values.length, values, min: sorted[0], median: quantile(0.5),
    p95: quantile(0.95), max: sorted.at(-1), context, thresholds: 'NONE; host budgets require agreement'}
}

export function feedbackLatencies(frames) {
  const seen = new Set(), times = []
  for (const frame of frames) {
    const point = frame.point
    if (!point || seen.has(point.time) || !Number.isFinite(point.time) || frame.time < point.time) continue
    if (frame.left !== point.x + 12 || frame.top !== point.y + 12) continue
    seen.add(point.time)
    times.push(frame.time - point.time)
  }
  return measuredSamples(times, 'Browser monotonic clock: delivered DOM/Tauri pointer update to first matching rendered-layout rAF sample; excludes upstream input queue latency')
}

export function cancelFeedbackTimings(samples) {
  const requested = samples.find(sample => Number.isFinite(sample.cancelRequestedAt))?.cancelRequestedAt
  assert.ok(Number.isFinite(requested), 'Actual trusted Cancel click must be observed')
  const acknowledgement = samples.find(sample => sample.time >= requested && sample.label === 'Cancelling…')
  const terminal = samples.find(sample => sample.time >= requested && !sample.active && !sample.visible && /Paste failed:.*cancel/i.test(sample.toast))
  assert.ok(acknowledgement && terminal, 'Missing actual cancellation feedback/completion timestamps')
  return {acknowledgement: measuredSamples([acknowledgement.time - requested],
    'Browser monotonic clock: actual trusted Cancel click to observed Cancelling label'),
  terminal: measuredSamples([terminal.time - requested],
    'Browser monotonic clock: actual trusted Cancel click to cleared activity/operation plus cancellation failure toast; excludes toast dwell and verifier polling; task/callback release checked separately')}
}
