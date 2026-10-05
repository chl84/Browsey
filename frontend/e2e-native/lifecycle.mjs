import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import net from 'node:net'
import { setTimeout as sleep } from 'node:timers/promises'
import { setTimeout, clearTimeout } from 'node:timers'

const children = new WeakMap()
const candidates = new WeakSet()

export class LifecycleError extends Error {
  constructor(kind, message) {
    super(message)
    this.failureKind = kind
  }
}

export async function bounded(action, milliseconds, kind, message) {
  let timer
  try {
    return await Promise.race([Promise.resolve().then(action), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new LifecycleError(kind, message)), milliseconds)
    })])
  } finally { clearTimeout(timer) }
}

export function trackChild(child) {
  if (children.has(child)) return children.get(child)
  let resolveExit
  const state = { error: null, exited: false, exit: new Promise(resolve => { resolveExit = resolve }) }
  child.once('error', error => {
    state.error = error
    if (!Number.isSafeInteger(child.pid)) { state.exited = true; resolveExit() }
  })
  child.once('exit', () => { state.exited = true; resolveExit() })
  if (child.exitCode !== null || child.signalCode !== null) { state.exited = true; resolveExit() }
  children.set(child, state)
  return state
}

export function assertDriverAlive(driver) {
  const state = trackChild(driver)
  if (!Number.isSafeInteger(driver.pid)) {
    throw new LifecycleError('DRIVER_SPAWN_FAILED', 'Owned driver could not be spawned')
  }
  if (state.error) throw new LifecycleError('DRIVER_ERROR', 'Owned driver reported a process error')
  if (state.exited || driver.exitCode !== null || driver.signalCode !== null) {
    throw new LifecycleError('DRIVER_EXITED', 'Owned driver exited before completion')
  }
}

export async function checkPorts(ports = [4444, 4445]) {
  const servers = []
  try {
    for (const port of ports) {
      const server = net.createServer()
      servers.push(server)
      await new Promise((resolve, reject) => {
        server.once('error', reject)
        server.listen(port, '127.0.0.1', resolve)
      })
    }
  } catch (error) {
    throw new LifecycleError('PORT_UNAVAILABLE', `Native driver port unavailable (${error.code ?? 'unknown'})`)
  } finally {
    await Promise.all(servers.filter(server => server.listening).map(server =>
      new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))))
  }
}

// Inspect only the owned process and its direct child, never discover services
// or attach to a listener which raced our preflight. Linux native testing only.
export async function ownsListener(pid, port) {
  const links = await Promise.all((await fs.readdir(`/proc/${pid}/fd`)).map(async fd => {
    try { return await fs.readlink(`/proc/${pid}/fd/${fd}`) } catch { return '' }
  }))
  const sockets = new Set(links.map(link => /^socket:\[(\d+)\]$/.exec(link)?.[1]).filter(Boolean))
  const tcp = (await Promise.all(['tcp', 'tcp6'].map(name => fs.readFile(`/proc/${pid}/net/${name}`, 'utf8')))).join('\n')
  return tcp.split('\n').slice(1).some(line => {
    const fields = line.trim().split(/\s+/)
    return fields[3] === '0A' && Number.parseInt(fields[1]?.split(':')[1], 16) === port && sockets.has(fields[9])
  })
}

export async function ownedNativeDriverPid(driver, webkitExecutable, port = 4444, nativePort = 4445) {
  if (!await ownsListener(driver.pid, port)) return null
  const pids = (await fs.readFile(`/proc/${driver.pid}/task/${driver.pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean)
  for (const raw of pids) {
    const pid = Number(raw)
    try {
      if (await fs.readlink(`/proc/${pid}/exe`) === webkitExecutable && await ownsListener(pid, nativePort)) return pid
    } catch (error) { if (error.code !== 'ENOENT' && error.code !== 'ESRCH') throw error }
  }
  return null
}

export async function waitDriver(driver, { timeoutMs = 15_000, intervalMs = 100,
  listening = () => ownsListener(driver.pid, 4444),
  probe = signal => fetch('http://127.0.0.1:4444/status', { signal }) } = {}) {
  const state = trackChild(driver)
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    assertDriverAlive(driver)
    try {
      if (await listening()) {
        const controller = new globalThis.AbortController()
        try {
          const response = await Promise.race([
            bounded(() => probe(controller.signal), Math.min(1000, Math.max(1, deadline - Date.now())),
              'DRIVER_PROBE_TIMEOUT', 'Owned driver readiness probe timed out'),
            state.exit.then(() => { assertDriverAlive(driver) }),
          ])
          assertDriverAlive(driver)
          if (response?.ok) return
        } finally { controller.abort() }
      }
    } catch (error) {
      assertDriverAlive(driver)
      if (error instanceof LifecycleError && error.failureKind !== 'DRIVER_PROBE_TIMEOUT') throw error
      // Readiness only: no session or file operation is retried here.
    }
    await Promise.race([sleep(Math.min(intervalMs, Math.max(1, deadline - Date.now()))), state.exit])
  }
  assertDriverAlive(driver)
  throw new LifecycleError('DRIVER_STARTUP_TIMEOUT', 'Owned driver did not become ready')
}

export async function stopDriver(driver, { termMs = 3000, killMs = 3000 } = {}) {
  const state = trackChild(driver)
  if (state.exited) return
  driver.kill('SIGTERM')
  try { await bounded(() => state.exit, termMs, 'DRIVER_TERM_TIMEOUT', 'Owned driver did not exit after TERM') }
  catch (error) {
    if (error.failureKind !== 'DRIVER_TERM_TIMEOUT') throw error
    if (!state.exited) driver.kill('SIGKILL')
    await bounded(() => state.exit, killMs, 'DRIVER_EXIT_UNCONFIRMED', 'Owned driver exit could not be confirmed')
  }
}

async function readIdentity(pid) {
  const stat = async () => {
    const raw = await fs.readFile(`/proc/${pid}/stat`, 'utf8')
    const fields = raw.slice(raw.lastIndexOf(')') + 2).split(' ')
    return { state: fields[0], start: fields[19] }
  }
  const before = await stat()
  if (['Z', 'X'].includes(before.state)) return { exited: true }
  const executable = await fs.readlink(`/proc/${pid}/exe`)
  const env = (await fs.readFile(`/proc/${pid}/environ`, 'utf8')).split('\0')
  const after = await stat()
  if (before.start !== after.start) throw new LifecycleError('CANDIDATE_IDENTITY_CHANGED', 'Candidate PID was reused during identity verification')
  if (['Z', 'X'].includes(after.state)) return { exited: true }
  let session
  try { session = JSON.parse(env.find(value => value.startsWith('BROWSEY_NATIVE_TEST_SESSION='))?.slice('BROWSEY_NATIVE_TEST_SESSION='.length)) }
  catch { /* Missing/invalid marker cannot prove ownership. */ }
  return { start: after.start, executable, dataHome: env.find(value => value.startsWith('XDG_DATA_HOME='))?.slice(14), runId: session?.runId }
}

export async function captureCandidate(pid, expected) {
  assert.ok(Number.isSafeInteger(pid) && pid > 1, 'Expected a candidate PID')
  const actual = await readIdentity(pid)
  if (actual.exited || !actual.start || actual.executable !== expected.executable
    || actual.dataHome !== expected.dataHome || actual.runId !== expected.runId) {
    throw new LifecycleError('CANDIDATE_IDENTITY_UNPROVEN', 'Refuse a candidate without matching executable, profile and run identity')
  }
  const owner = Object.freeze({ pid, ...expected, start: actual.start })
  candidates.add(owner)
  return owner
}

export async function candidateState(owner, inspect = readIdentity) {
  assert.ok(candidates.has(owner), 'Candidate ownership must be captured before inspection or signals')
  try {
    const actual = await inspect(owner.pid)
    if (actual.exited || actual.start !== owner.start) return 'exited'
    if (actual.executable !== owner.executable || actual.dataHome !== owner.dataHome || actual.runId !== owner.runId) return 'changed'
    return 'alive'
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ESRCH') return 'exited'
    throw error
  }
}

export async function assertCandidateAlive(owner) {
  const state = await candidateState(owner)
  if (state !== 'alive') throw new LifecycleError(state === 'exited'
    ? owner.role === 'native-driver' ? 'NATIVE_DRIVER_EXITED' : 'CANDIDATE_EXITED' : 'CANDIDATE_IDENTITY_CHANGED',
    'Owned candidate exited or changed identity before completion')
}

async function waitCandidateExit(owner, milliseconds, inspect) {
  const deadline = Date.now() + milliseconds
  while (Date.now() < deadline) {
    const state = await bounded(() => candidateState(owner, inspect), Math.max(1, deadline - Date.now()),
      'CANDIDATE_EXIT_UNCONFIRMED', 'Owned candidate exit could not be confirmed')
    if (state === 'exited') return
    if (state !== 'alive') throw new LifecycleError('CANDIDATE_IDENTITY_CHANGED', 'Refuse to signal a changed candidate process')
    await sleep(Math.min(50, Math.max(1, deadline - Date.now())))
  }
  throw new LifecycleError('CANDIDATE_EXIT_UNCONFIRMED', 'Owned candidate exit could not be confirmed')
}

export async function stopCandidate(owner, { termMs = 1500, killMs = 1500, inspect = readIdentity,
  signal = (pid, name) => process.kill(pid, name) } = {}) {
  const send = async name => {
    const state = await candidateState(owner, inspect)
    if (state === 'exited') return false
    if (state !== 'alive') throw new LifecycleError('CANDIDATE_IDENTITY_CHANGED', 'Refuse to signal a changed candidate process')
    try { signal(owner.pid, name) } catch (error) { if (error.code !== 'ESRCH') throw error }
    return true
  }
  if (!await send('SIGTERM')) return
  try { await waitCandidateExit(owner, termMs, inspect) }
  catch (error) {
    if (error.failureKind !== 'CANDIDATE_EXIT_UNCONFIRMED') throw error
    if (await send('SIGKILL')) await waitCandidateExit(owner, killMs, inspect)
  }
}

export async function teardown({ browser, driver, nativeDriver, candidate, closeMs = 5000, driverLimits, candidateLimits }) {
  const steps = []
  const step = async (stage, action) => {
    try { await action(); steps.push({ stage, status: 'PASS' }) }
    catch (error) { steps.push({ stage, status: 'BLOCKED', failureKind: error.failureKind
      ?? (stage === 'session-close' ? 'SESSION_CLOSE_FAILED' : 'TEARDOWN_FAILED'), error: error.message }) }
  }
  if (browser) await step('session-close', () => bounded(() => browser.deleteSession(), closeMs,
    'SESSION_CLOSE_TIMEOUT', 'WebDriver session closure timed out; no retry'))
  if (driver) await step('driver-exit', () => stopDriver(driver, driverLimits))
  if (nativeDriver) await step('native-driver-exit', () => stopCandidate(nativeDriver, candidateLimits))
  if (candidate) await step('candidate-exit', () => stopCandidate(candidate, candidateLimits))
  else if (browser) steps.push({ stage: 'candidate-exit', status: 'BLOCKED', failureKind: 'CANDIDATE_IDENTITY_UNPROVEN',
    error: 'Candidate exit cannot be confirmed without captured ownership' })
  return { status: steps.some(step => step.status !== 'PASS') ? 'BLOCKED' : 'PASS', steps }
}

export function applyTeardown(report, result) {
  report.teardownDetails = result
  report.teardown = result.status === 'PASS' ? 'PASS: owned candidate and driver exited'
    : 'BLOCKED: owned teardown or session closure could not be confirmed'
  if (result.status !== 'PASS' && report.status === 'PASS') report.status = 'BLOCKED'
}
