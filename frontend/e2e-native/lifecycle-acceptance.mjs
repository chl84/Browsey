import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { repo, faultReport } from './acceptance.mjs'

const expectations = {
  'driver-exit': 'DRIVER_EXITED', 'candidate-exit': 'CANDIDATE_EXITED', 'case-timeout': 'CASE_TIMEOUT',
  'session-close': 'SESSION_CLOSE_FAILED', 'session-timeout': 'SESSION_CLOSE_TIMEOUT',
}

// Explicit opt-in native regressions. Each fresh local run has no UI file
// mutations; FAIL/BLOCKED is expected only for the exact injected failure.
async function main() {
  assert.equal(process.argv.length, 2, 'Lifecycle acceptance takes no target or credential overrides')
  const results = []
  for (const [fault, expected] of Object.entries(expectations)) {
    const { report, raw } = await faultReport('--lifecycle-fault', fault, expected)
    const exitSteps = report.teardownDetails.steps.filter(step => step.stage !== 'session-close')
    assert.ok(exitSteps.length && exitSteps.every(step => step.status === 'PASS'), 'All captured owned processes must exit')
    results.push({ fault, expected, status: report.status, report: raw, build: report.build, harnessSha256: report.harnessSha256 })
    console.log(`PASS lifecycle regression: ${fault}; native report ${report.status}, owned processes exited`)
  }
  await fs.writeFile(path.join(repo, 'target/native-test/lifecycle-results.json'), JSON.stringify(results, null, 2), { mode: 0o600 })
  console.log('Five native lifecycle fault regressions passed; private reports and fixtures retained.')
}

main().catch(error => { console.error(`Native lifecycle acceptance stopped: ${error.message}`); process.exitCode = 1 })
