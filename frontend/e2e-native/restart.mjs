import assert from 'node:assert/strict'

// One deliberate restart of the already owned candidate/session. A failed
// closure cannot become permission to launch another candidate or retry.
export function ownedRestart({ stop, start, persist, restarts }) {
  let attempted = false
  return async () => {
    assert.equal(attempted, false, 'Only one explicit owned restart is allowed')
    attempted = true
    const evidence = { status: 'RUNNING', started: new Date().toISOString() }
    restarts.push(evidence); await persist()
    try {
      evidence.closed = await stop()
      assert.equal(evidence.closed.teardown.status, 'PASS', 'Refuse restart after unconfirmed owned teardown')
      evidence.opened = await start()
      evidence.status = 'PASS'
    } catch (error) { evidence.status = 'BLOCKED'; throw error }
    finally { evidence.finished = new Date().toISOString(); await persist() }
  }
}
