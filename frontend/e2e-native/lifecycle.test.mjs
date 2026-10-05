import assert from 'node:assert/strict'
import { test } from 'node:test'
import { spawn } from 'node:child_process'
import { once, EventEmitter } from 'node:events'
import * as fs from 'node:fs/promises'
import net from 'node:net'
import { setTimeout } from 'node:timers'
import { randomUUID } from 'node:crypto'
import { bounded, trackChild, checkPorts, ownsListener, waitDriver, assertDriverAlive, stopDriver,
  captureCandidate, candidateState, assertCandidateAlive, stopCandidate, teardown, applyTeardown } from './lifecycle.mjs'

const matches = kind => error => error.failureKind === kind
const idle = "process.send({ready:true}); setInterval(() => {}, 1000)"
const resistant = "process.on('SIGTERM', () => {}); " + idle

async function ownProcess(t, source = idle) {
  const runId = randomUUID()
  const dataHome = `/generated-native-lifecycle/${runId}/data`
  const child = spawn(process.execPath, ['-e', source], {
    env: { XDG_DATA_HOME: dataHome, BROWSEY_NATIVE_TEST_SESSION: JSON.stringify({ runId }) },
    stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
  })
  trackChild(child)
  t.after(() => stopDriver(child, { termMs: 50, killMs: 1500 }))
  const [message] = await bounded(() => once(child, 'message'), 2500, 'TEST_SETUP_TIMEOUT', 'Generated child did not start')
  return { child, message, expected: { executable: await fs.realpath(process.execPath), dataHome, runId } }
}

async function server(t) {
  const listener = net.createServer()
  listener.listen(0, '127.0.0.1')
  await once(listener, 'listening')
  t.after(() => new Promise(resolve => listener.close(resolve)))
  return listener
}

test('occupied ports reject before any probe, release earlier reservations and leave the listener alive', async t => {
  const held = await server(t)
  const first = await server(t)
  const firstPort = first.address().port
  await new Promise(resolve => first.close(resolve))
  await assert.rejects(checkPorts([firstPort, held.address().port]), matches('PORT_UNAVAILABLE'))
  assert.equal(held.listening, true)
  const rebound = net.createServer()
  rebound.listen(firstPort, '127.0.0.1')
  await once(rebound, 'listening')
  await new Promise(resolve => rebound.close(resolve))
})

test('a foreign listener racing preflight is never probed or accepted as the owned driver', async t => {
  const foreign = await server(t)
  const { child } = await ownProcess(t)
  let probes = 0
  await assert.rejects(waitDriver(child, { timeoutMs: 80, intervalMs: 10,
    listening: () => ownsListener(child.pid, foreign.address().port),
    probe: async () => { probes++; return { ok: true } },
  }), matches('DRIVER_STARTUP_TIMEOUT'))
  assert.equal(probes, 0)
  assert.equal(foreign.listening, true)
})

test('an owned loopback driver becomes ready only after proving its listener identity', async t => {
  const source = "const http=require('node:http'); const server=http.createServer((_,res)=>res.end('{}')); server.listen(0,'127.0.0.1',()=>process.send({port:server.address().port}))"
  const { child, message } = await ownProcess(t, source)
  await waitDriver(child, { timeoutMs: 1500, listening: () => ownsListener(child.pid, message.port),
    probe: signal => fetch(`http://127.0.0.1:${message.port}/status`, { signal }) })
  assertDriverAlive(child)
})

test('spawn errors and already-exited drivers fail without readiness requests', async t => {
  const missing = spawn('/nonexistent/browsey-native-test-driver', [], { stdio: 'ignore' })
  const failed = trackChild(missing)
  await failed.exit
  let probes = 0
  await assert.rejects(waitDriver(missing, { probe: () => { probes++; return { ok: true } } }), matches('DRIVER_SPAWN_FAILED'))
  await stopDriver(missing)
  for (const signal of [null, 'SIGKILL']) {
    const { child } = await ownProcess(t, "process.on('message', () => process.exit(0)); " + idle)
    if (signal) child.kill(signal)
    else child.send('exit')
    await trackChild(child).exit
    await assert.rejects(waitDriver(child, { probe: () => { probes++; return { ok: true } } }), matches('DRIVER_EXITED'))
  }
  assert.equal(probes, 0)
})

test('driver death interrupts a hung readiness probe and startup timeout remains bounded', async t => {
  const { child } = await ownProcess(t)
  let probes = 0
  const started = Date.now()
  await assert.rejects(waitDriver(child, { timeoutMs: 1500, listening: async () => true,
    probe: () => { probes++; setTimeout(() => child.kill('SIGKILL'), 20); return new Promise(() => {}) },
  }), matches('DRIVER_EXITED'))
  assert.equal(probes, 1)
  assert.ok(Date.now() - started < 1000, 'Exit must not wait for the whole startup deadline')
  const { child: neverReady } = await ownProcess(t)
  await assert.rejects(waitDriver(neverReady, { timeoutMs: 80, intervalMs: 10, listening: async () => true,
    probe: () => new Promise(() => {}) }), matches('DRIVER_STARTUP_TIMEOUT'))
})

test('TERM-resistant owned driver escalates to KILL while an unrelated sentinel remains alive', async t => {
  const { child } = await ownProcess(t, resistant)
  const { child: sentinel } = await ownProcess(t)
  await stopDriver(child, { termMs: 40, killMs: 1500 })
  assert.equal(child.signalCode, 'SIGKILL')
  assertDriverAlive(sentinel)
})

test('candidate capture checks executable, private profile and run before granting signal ownership', async t => {
  const { child, expected } = await ownProcess(t)
  for (const override of [{ executable: '/different/candidate' }, { dataHome: '/different/profile' }, { runId: 'different-run' }]) {
    await assert.rejects(captureCandidate(child.pid, { ...expected, ...override }), matches('CANDIDATE_IDENTITY_UNPROVEN'))
  }
  const owner = await captureCandidate(child.pid, expected)
  assert.ok(owner.start)
  assert.equal(await candidateState(owner), 'alive')
  await assert.rejects(stopCandidate({ ...owner }), /ownership must be captured/)
  assertDriverAlive(child)
})

test('candidate crash is detected and shutdown never signals a reused PID or changed identity', async t => {
  const { child, expected } = await ownProcess(t)
  const owner = await captureCandidate(child.pid, expected)
  const signals = []
  await stopCandidate(owner, { inspect: async () => ({ ...expected, start: `${owner.start}-reused` }),
    signal: (...args) => signals.push(args) })
  await assert.rejects(stopCandidate(owner, { inspect: async () => ({ ...expected, start: owner.start, dataHome: '/changed/profile' }),
    signal: (...args) => signals.push(args) }), matches('CANDIDATE_IDENTITY_CHANGED'))
  assert.deepEqual(signals, [])
  assertDriverAlive(child)
  child.kill('SIGKILL')
  await trackChild(child).exit
  await assert.rejects(assertCandidateAlive(owner), matches('CANDIDATE_EXITED'))
  await stopCandidate(owner)
})

test('session-close failure and timeout still stop all three owned processes and block would-be PASS', async t => {
  for (const timeout of [false, true]) {
    const { child: driver } = await ownProcess(t)
    const { child: nativeDriver, expected: nativeExpected } = await ownProcess(t, resistant)
    const nativeOwner = await captureCandidate(nativeDriver.pid, { ...nativeExpected, role: 'native-driver' })
    const { child: candidate, expected } = await ownProcess(t, resistant)
    const owner = await captureCandidate(candidate.pid, expected)
    let closeCalls = 0
    const result = await teardown({ driver, nativeDriver: nativeOwner, candidate: owner, closeMs: 40,
      candidateLimits: { termMs: 40, killMs: 1500 },
      browser: { deleteSession: () => { closeCalls++; return timeout ? new Promise(() => {}) : Promise.reject(new Error('Injected closure failure')) } },
    })
    assert.equal(closeCalls, 1, 'Uncertain session closure must not be resent')
    assert.equal(result.status, 'BLOCKED')
    assert.equal(result.steps[0].failureKind, timeout ? 'SESSION_CLOSE_TIMEOUT' : 'SESSION_CLOSE_FAILED')
    assert.deepEqual(result.steps.slice(1).map(step => step.status), ['PASS', 'PASS', 'PASS'])
    assert.equal(trackChild(driver).exited, true)
    await trackChild(nativeDriver).exit
    assert.equal(nativeDriver.signalCode, 'SIGKILL')
    await trackChild(candidate).exit
    assert.equal(candidate.signalCode, 'SIGKILL')
    const report = { status: 'PASS' }
    applyTeardown(report, result)
    assert.equal(report.status, 'BLOCKED')
    const failedReport = { status: 'FAIL' }
    applyTeardown(failedReport, result)
    assert.equal(failedReport.status, 'FAIL')
  }
})

test('unconfirmed driver/candidate exits preserve every teardown failure and never become PASS', async t => {
  const driver = Object.assign(new EventEmitter(), { pid: 12345, exitCode: null, signalCode: null, kill: () => true })
  const { child, expected } = await ownProcess(t)
  const owner = await captureCandidate(child.pid, expected)
  const signals = []
  const result = await teardown({ driver, candidate: owner, driverLimits: { termMs: 20, killMs: 20 },
    candidateLimits: { termMs: 20, killMs: 20, inspect: async () => ({ ...expected, start: owner.start }),
      signal: (_, name) => signals.push(name) },
    browser: { deleteSession: () => Promise.reject(new Error('Injected closure failure')) },
  })
  assert.equal(result.status, 'BLOCKED')
  assert.deepEqual(result.steps.map(step => step.status), ['BLOCKED', 'BLOCKED', 'BLOCKED'])
  assert.deepEqual(signals, ['SIGTERM', 'SIGKILL'])
  assertDriverAlive(child)
  const report = { status: 'PASS' }
  applyTeardown(report, result)
  assert.equal(report.status, 'BLOCKED')
})

test('normal shutdown confirms owned exits, and timed-out read-only actions are never retried', async t => {
  const { child: driver } = await ownProcess(t)
  const { child: candidate, expected } = await ownProcess(t)
  const owner = await captureCandidate(candidate.pid, expected)
  const result = await teardown({ driver, candidate: owner, browser: { deleteSession: async () => {
    candidate.kill('SIGTERM'); await trackChild(candidate).exit
  } } })
  assert.equal(result.status, 'PASS')
  const report = { status: 'PASS' }
  applyTeardown(report, result)
  assert.equal(report.status, 'PASS')
  let calls = 0
  await assert.rejects(bounded(() => { calls++; return new Promise(() => {}) }, 20, 'CASE_TIMEOUT', 'Generated read-only timeout'), matches('CASE_TIMEOUT'))
  assert.equal(calls, 1)
})
