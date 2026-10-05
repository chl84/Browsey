import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { validateConfig, noLinks } from './scope.mjs'
import { regularFile } from './fixtures.mjs'

const repo = fileURLToPath(new URL('../..', import.meta.url))
const exec = promisify(execFile)
const expectations = {
  'driver-exit': 'DRIVER_EXITED', 'candidate-exit': 'CANDIDATE_EXITED', 'case-timeout': 'CASE_TIMEOUT',
  'session-close': 'SESSION_CLOSE_FAILED', 'session-timeout': 'SESSION_CLOSE_TIMEOUT',
}

// Explicit opt-in native regressions. Each fresh local run has no UI file
// mutations; FAIL/BLOCKED is expected only for the exact injected failure.
async function main() {
  assert.equal(process.argv.length, 2, 'Lifecycle acceptance takes no target or credential overrides')
  const config = validateConfig(JSON.parse((await regularFile(path.join(repo, 'frontend/e2e-native/config.local.json'))).text))
  const local = config.targets.find(target => target.kind === 'local')
  const results = []
  for (const [fault, expected] of Object.entries(expectations)) {
    let output, code
    try {
      output = (await exec(process.execPath, [path.join(repo, 'frontend/e2e-native/run.mjs'), '--run', '--targets', 'local',
        '--lifecycle-fault', fault], { cwd: repo, maxBuffer: 1024 * 1024 })).stdout
      code = 0
    } catch (error) { output = error.stdout; code = error.code }
    assert.equal(code, 1, 'Injected failure must stop the runner with exit 1')
    const raw = /^Native report: (.+)$/m.exec(output)?.[1]
    assert.ok(raw && /^\.bnt-[a-f0-9]{32}\/report\.json$/.test(path.relative(local.path, raw)), 'Read only this generated owned report')
    await noLinks(raw, fs)
    const report = JSON.parse((await regularFile(raw)).text)
    const owner = JSON.parse((await regularFile(path.join(path.dirname(raw), 'owner.json'))).text)
    assert.equal(owner.runId, report.runId)
    assert.equal(path.basename(path.dirname(raw)), `.bnt-${report.runId.replaceAll('-', '')}`)
    assert.equal(report.injectedLifecycleFault, fault)
    assert.ok(['FAIL', 'BLOCKED'].includes(report.status), 'Expected failure cannot be a native PASS')
    const observed = report.failureKind ?? report.teardownDetails?.steps.find(step => step.status !== 'PASS')?.failureKind
    assert.equal(observed, expected, 'Unexpected failure is not successful fault acceptance')
    const exitSteps = report.teardownDetails.steps.filter(step => step.stage !== 'session-close')
    assert.ok(exitSteps.length && exitSteps.every(step => step.status === 'PASS'), 'All captured owned processes must exit')
    for (const pid of [report.driverIdentity?.pid, report.driverIdentity?.nativePid, report.identity?.pid].filter(Boolean)) {
      await assert.rejects(fs.readlink(`/proc/${pid}/exe`), error => error.code === 'ENOENT', 'Owned process must be gone')
    }
    results.push({ fault, expected, status: report.status, report: raw, build: report.build, harnessSha256: report.harnessSha256 })
    console.log(`PASS lifecycle regression: ${fault}; native report ${report.status}, owned processes exited`)
  }
  await fs.writeFile(path.join(repo, 'target/native-test/lifecycle-results.json'), JSON.stringify(results, null, 2), { mode: 0o600 })
  console.log('Five native lifecycle fault regressions passed; private reports and fixtures retained.')
}

main().catch(error => { console.error(`Native lifecycle acceptance stopped: ${error.message}`); process.exitCode = 1 })
