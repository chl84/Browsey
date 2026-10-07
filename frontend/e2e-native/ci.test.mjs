import assert from 'node:assert/strict'
import {test} from 'node:test'
import {approvedCiEnvironment, validateCiToolchain, redactedCiReport} from './ci.mjs'
const env = {GITHUB_ACTIONS: 'true', GITHUB_REF: 'refs/heads/main', BROWSEY_NATIVE_CI_EPHEMERAL: 'yes', BROWSEY_NATIVE_CI_IMAGE_ID: 'fixture-image', RUNNER_TEMP: '/tmp/bci'}
test('CI scope requires explicit disposable main image, short root and no inherited desktop/agent', () => {
  assert.deepEqual(approvedCiEnvironment(env), {schema: 1, targets: {local: '/tmp/bci/ai_agent_testfolder'}, rcloneConfig: null})
  for (const patch of [{GITHUB_ACTIONS: ''}, {GITHUB_REF: 'refs/heads/untrusted'}, {BROWSEY_NATIVE_CI_EPHEMERAL: ''}, {BROWSEY_NATIVE_CI_IMAGE_ID: ''}, {RUNNER_TEMP: '/tmp/' + 'long'.repeat(40)}, {DISPLAY: ':0'}, {DBUS_SESSION_BUS_ADDRESS: 'personal'}, {SSH_AUTH_SOCK: '/agent'}])
    assert.throws(() => approvedCiEnvironment({...env, ...patch}))
})
test('CI toolchain refuses mismatched driver/app engines, GTK, architecture or hashes', () => {
  const actual = {arch: 'x64', gtk: '3.24', webkit: '2.52', tools: Object.fromEntries(['tauri-driver', 'WebKitWebDriver', 'Xvfb', 'bwrap'].map(name => [name, 'a'.repeat(64)]))}
  const spec = {schema: 1, imageId: 'fixture-image', driverWebkit: '2.52', ...actual}
  assert.equal(validateCiToolchain(spec, actual).imageId, 'fixture-image')
  for (const patch of [{driverWebkit: '2.50'}, {gtk: '3.22'}, {webkit: '2.50'}, {arch: 'arm64'}, {tools: {...actual.tools, Xvfb: 'b'.repeat(64)}}])
    assert.throws(() => validateCiToolchain({...spec, ...patch}, actual))
})
test('redacted CI receipt excludes private fields and never promotes missing native parts/teardown to PASS', () => {
  const report = {schema: 3, suite: 'smoke', evidence: {kind: 'REAL_NATIVE'}, runId: '00000000-0000-4000-8000-000000000000', status: 'PASS',
    build: {commit: 'a'.repeat(40), privatePath: '/personal'}, harnessSha256: 'b'.repeat(64), host: {platform: 'linux', privateName: 'secret-person'}, tools: {credentials: 'secret-token'},
    cases: ['input-local', 'copy-within-local', 'undo-copy-local', 'accessibility-local'].map(id => ({id, status: 'PASS', parts: (id === 'copy-within-local' ? ['file', 'directory'] : []).map(id => ({id, status: 'PASS', path: '/personal'})), error: 'secret-token'})),
    teardownDetails: {status: 'PASS', steps: ['session-close', 'driver-exit', 'native-driver-exit', 'candidate-exit'].map(stage => ({stage, status: 'PASS'}))}, desktop: {teardown: 'PASS'}, retention: {status: 'PASS'}, profile: '/personal'}
  const result = redactedCiReport(report)
  assert.equal(result.status, 'PASS')
  assert.ok(!/secret|personal|credentials|privateName/.test(JSON.stringify(result)))
  const missing = structuredClone(report); missing.cases[1].parts[1].status = 'NOT_RUN'
  assert.equal(redactedCiReport(missing).status, 'BLOCKED')
  const duplicate = structuredClone(report); duplicate.teardownDetails.steps[3].stage = 'driver-exit'
  assert.equal(redactedCiReport(duplicate).status, 'BLOCKED')
  assert.throws(() => redactedCiReport({...report, evidence: {kind: 'MOCK'}}))
  assert.throws(() => redactedCiReport({...report, suite: 'cloud-provider'}))
})
/* global structuredClone */
