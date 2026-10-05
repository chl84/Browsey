import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { faultReport, preflightFailure, repo } from './acceptance.mjs'
import { regularFile, payload } from './fixtures.mjs'

// Explicit local reporting regressions, never an app-defect reproduction.
async function main() {
  assert.equal(process.argv.length, 2, 'Report acceptance takes no scope overrides')
  const results = []
  const preflight = await preflightFailure()
  results.push({ fault: 'missing-driver-preflight', status: preflight.status, report: null, harnessSha256: preflight.harnessSha256 })
  console.log('PASS reporting regression: missing-driver-preflight; structured BLOCKED, no owned run or UI cases')
  for (const [fault, expected] of [['setup-failure', 'INJECTED_SETUP'], ['partial-transfer', 'FIXTURE_IO']]) {
    const { report, raw } = await faultReport('--report-fault', fault, expected)
    assert.equal(report.status, 'BLOCKED')
    assert.equal(report.failureOrigin, 'harness')
    assert.equal(report.teardownDetails.status, 'PASS')
    assert.equal(report.providers.local.status, 'BLOCKED')
    assert.ok(report.cases.every(item => item.id && item.requirements.local.length))
    if (fault === 'setup-failure') {
      assert.equal(report.setup.find(item => item.id === 'private-profile').status, 'BLOCKED')
      assert.ok(report.cases.every(item => item.status === 'NOT_RUN'))
      assert.equal(report.identity, undefined)
      assert.equal(report.driverIdentity, undefined)
      assert.equal(report.teardownDetails.steps.length, 0)
    } else {
      const transfer = report.cases.find(item => item.id === 'copy-within-local')
      assert.deepEqual(transfer.parts.map(item => item.status), ['PASS', 'BLOCKED'])
      assert.ok(transfer.parts.every(item => item.ui === 'ACKNOWLEDGED'))
      assert.equal(report.providers.local.counts.PASS, 4)
      assert.equal(report.providers.local.counts.BLOCKED, 1)
      assert.equal(report.providers.local.counts.NOT_RUN, 2)
      assert.equal(report.cases.find(item => item.id === 'move-within-local').status, 'NOT_RUN')
      assert.equal(report.cases.find(item => item.id === 'undo-copy-local').status, 'NOT_RUN')
      assert.equal(report.setup.find(item => item.id === 'candidate-identity').status, 'PASS')
      assert.equal(report.teardownDetails.steps.length, 4)
      // Independently inspect only these generated source/destination fixtures.
      const files = path.join(path.dirname(raw), 'files')
      for (const side of ['source', 'target']) for (const leaf of ['sample.txt', 'tree/nested.txt']) {
        assert.equal((await regularFile(`${files}/copy-within-local-${side}/${leaf}`)).text, payload)
      }
    }
    results.push({ fault, expected, status: report.status, report: raw, build: report.build, harnessSha256: report.harnessSha256 })
    console.log(`PASS reporting regression: ${fault}; native report BLOCKED, completed/unrun work preserved`)
  }
  await fs.writeFile(path.join(repo, 'target/native-test/report-results.json'), JSON.stringify(results, null, 2), { mode: 0o600 })
}

main().catch(error => { console.error(`Native report acceptance stopped: ${error.message}`); process.exitCode = 1 })
