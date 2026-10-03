// Opt-in candidate startup measurement. No personal app restart or browser substitute.
import assert from 'node:assert/strict'
import { execFileSync, spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statfsSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { once } from 'node:events'

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const candidate = join(repo, 'target/release/browsey')
assert.equal(process.env.BROWSEY_NATIVE_PERFORMANCE_APPROVED, 'yes', 'Explicit opt-in to isolated native windows is required')
assert.equal(process.platform, 'linux')
assert.ok(existsSync(candidate), 'Build the production candidate with Tauri first; this script does not install or build it')
assert.equal(realpathSync(candidate), candidate, 'Candidate path must not redirect to an installed app')
const parent = process.env.XDG_STATE_HOME ?? join(process.env.HOME, '.local/state')
const space = statfsSync(parent)
assert.ok(space.bavail * space.bsize > 1024 ** 3, 'At least 1 GiB of disposable fixture space is required')
const root = mkdtempSync(join(parent, 'browsey-native-perf-'))
const owner = randomUUID()
writeFileSync(join(root, 'owner.txt'), owner)
let running
let success = false
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
const summarize = samples => {
  const sorted = [...samples].sort((a, b) => a - b)
  return { samplesMs: samples, medianMs: sorted[Math.floor(sorted.length / 2)], maxMs: sorted.at(-1) }
}
const processTreeRss = pid => {
  const rows = execFileSync('ps', ['-eo', 'pid=,ppid=,rss='], { encoding: 'utf8' })
    .trim().split('\n').map(line => line.trim().split(/\s+/).map(Number))
  const owned = new Set([pid])
  for (let previous = -1; previous !== owned.size;) {
    previous = owned.size
    for (const [child, parent] of rows) if (owned.has(parent)) owned.add(child)
  }
  return { processes: owned.size, sumRssKiB: rows.filter(([child]) => owned.has(child)).reduce((sum, row) => sum + row[2], 0),
    caveat: 'sum of parent/descendant RSS, shared pages can be counted more than once' }
}
try {
  for (const count of [10_000, 100_000]) {
    const directory = join(root, `entries-${count}`)
    const profile = join(root, `profile-${count}`)
    mkdirSync(directory)
    mkdirSync(profile)
    for (let index = 0; index < count; index++) {
      const tag = index % 1000 === 0 ? 'needle' : 'ordinary'
      writeFileSync(join(directory, `item-${String(index).padStart(6, '0')}-${tag}.txt`), 'fixture\n')
    }
    const samples = []
    const memory = []
    for (let sample = 0; sample < 6; sample++) {
      const start = performance.now()
      running = spawn(candidate, [directory], {
        env: { ...process.env, XDG_DATA_HOME: join(profile, 'data'), XDG_CONFIG_HOME: join(profile, 'config'),
          XDG_CACHE_HOME: join(profile, 'cache'), XDG_STATE_HOME: join(profile, 'state'),
          BROWSEY_UNDO_DIR: join(profile, 'data/browsey/undo-sessions'), RUST_LOG: 'warn',
          NO_AT_BRIDGE: '0' },
        stdio: 'ignore',
      })
      const pid = running.pid
      assert.ok(pid)
      const readiness = JSON.parse(execFileSync('/usr/bin/python', [join(repo, 'tests/support/native_performance_ready.py'), String(pid), candidate],
        { encoding: 'utf8', timeout: 35_000 }))
      assert.equal(readiness.ready, true)
      samples.push(performance.now() - start)
      memory.push({ parent: readFileSync(`/proc/${pid}/status`, 'utf8').split('\n').filter(line => /^Vm(RSS|HWM):/.test(line)),
        processTree: processTreeRss(pid) })
      // This instance only listed generated text files. No user file operation runs.
      assert.equal(realpathSync(`/proc/${pid}/exe`), candidate)
      const exited = once(running, 'exit')
      // Normal exit runs shutdown hooks for the test instance's mount monitor.
      execFileSync('/usr/bin/python', [join(repo, 'tests/support/native_fixture_a11y.py'),
        String(pid), candidate, join(profile, 'data'), 'click', 'button', 'Close window'],
        { timeout: 10_000, env: { ...process.env, BROWSEY_TEST_A11Y_TIMEOUT: '5' }, stdio: 'ignore' })
      await Promise.race([exited, pause(5000)])
      assert.ok(running.exitCode !== null || running.signalCode !== null,
        'Candidate did not exit; leave fixture and do not start another instance')
      running = undefined
    }
    console.log(JSON.stringify({ schema: 1, label: 'native-candidate-launch-to-first-accessible-row', entries: count,
      firstPrivateProfileMs: samples[0], repeatedProfile: summarize(samples.slice(1)), memory,
      source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim(),
      sourceDirty: Boolean(execFileSync('git', ['status', '--porcelain'], { cwd: repo, encoding: 'utf8' }).trim()),
      candidateSha256: createHash('sha256').update(readFileSync(candidate)).digest('hex'),
      scope: 'production candidate plus AT-SPI readiness overhead; process restarts, generated OS-warm fixtures; not cold-disk or first-thumbnail timing' }))
  }
  success = true
} finally {
  if (running?.pid && existsSync(`/proc/${running.pid}/exe`)
      && realpathSync(`/proc/${running.pid}/exe`) === candidate) {
    running.kill('SIGTERM')
    await pause(1000)
  }
  if (success && readFileSync(join(root, 'owner.txt'), 'utf8') === owner) rmSync(root, { recursive: true })
  else console.error(`Owned native performance fixture retained for inspection: ${root}`)
}
