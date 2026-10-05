import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { validateConfig, noLinks } from './scope.mjs'
import { regularFile } from './fixtures.mjs'

export const repo = fileURLToPath(new URL('../..', import.meta.url))
const exec = promisify(execFile)

export async function preflightFailure() {
  let output, code
  try {
    output = (await exec(process.execPath, [path.join(repo, 'frontend/e2e-native/run.mjs'), '--run', '--targets', 'local'],
      { cwd: repo, maxBuffer: 1024 * 1024, env: { ...process.env, BROWSEY_TAURI_DRIVER: '/nonexistent/browsey-report-test-driver' } })).stdout
    code = 0
  } catch (error) { output = error.stdout; code = error.code }
  assert.equal(code, 1)
  assert.ok(!/^Native report:/m.test(output), 'Preflight must not claim an owned report path')
  const report = JSON.parse(/^Native preflight report: (.+)$/m.exec(output)?.[1] ?? '{}')
  assert.equal(report.schema, 3)
  assert.equal(report.status, 'BLOCKED')
  assert.equal(report.failureOrigin, 'harness')
  assert.equal(report.setup.find(item => item.id === 'dependencies')?.status, 'BLOCKED')
  assert.ok(report.cases.length && report.cases.every(item => item.status === 'NOT_RUN'))
  assert.equal(report.identity, undefined)
  assert.equal(report.driverIdentity, undefined)
  assert.match(report.harnessSha256, /^[a-f0-9]{64}$/)
  return report
}

export async function faultReport(flag, name, expected) {
  assert.ok(['--lifecycle-fault', '--report-fault'].includes(flag))
  const config = validateConfig(JSON.parse((await regularFile(path.join(repo, 'frontend/e2e-native/config.local.json'))).text))
  const local = config.targets.find(target => target.kind === 'local')
  let output, code
  try {
    output = (await exec(process.execPath, [path.join(repo, 'frontend/e2e-native/run.mjs'), '--run', '--targets', 'local', flag, name],
      { cwd: repo, maxBuffer: 1024 * 1024 })).stdout
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
  assert.equal(flag === '--lifecycle-fault' ? report.injectedLifecycleFault : report.injectedReportFault, name)
  assert.ok(['FAIL', 'BLOCKED'].includes(report.status), 'Expected failure cannot be a native PASS')
  const observed = report.failureKind ?? report.teardownDetails?.steps.find(step => step.status !== 'PASS')?.failureKind
  assert.equal(observed, expected, 'Unexpected failure is not successful fault acceptance')
  for (const pid of [report.driverIdentity?.pid, report.driverIdentity?.nativePid, report.identity?.pid].filter(Boolean)) {
    await assert.rejects(fs.readlink(`/proc/${pid}/exe`), error => error.code === 'ENOENT', 'Owned process must be gone')
  }
  return { report, raw }
}
