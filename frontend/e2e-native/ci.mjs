// Opt-in ephemeral Linux CI contract. No provisioning, accounts or device discovery.
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import {execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {randomUUID} from 'node:crypto'
import {fileURLToPath} from 'node:url'
import {fileSha256} from './candidate.mjs'
import {acceptanceRows} from './acceptance-map.mjs'
import {noLinks, validatePath, validateConfig, makePlan} from './scope.mjs'
import {privateJson, privateStat, writePrivate} from './privacy.mjs'
const exec = promisify(execFile)
const repo = fileURLToPath(new URL('../..', import.meta.url))
const toolNames = ['tauri-driver', 'WebKitWebDriver', 'Xvfb', 'bwrap']

export function validateCiToolchain(value, actual) {
  assert.equal(value.schema, 1)
  assert.match(value.imageId, /^[a-zA-Z0-9_.-]{1,100}$/)
  assert.equal(value.arch, actual.arch)
  assert.equal(value.gtk, actual.gtk, 'GTK version mismatch')
  assert.equal(value.webkit, actual.webkit, 'App WebKit version mismatch')
  assert.equal(value.driverWebkit, value.webkit, 'Driver/app WebKit engine versions must match')
  for (const name of toolNames) {
    assert.match(value.tools[name], /^[a-f0-9]{64}$/)
    assert.equal(value.tools[name], actual.tools[name], 'Native tool hash mismatch')
  }
  return {imageId: value.imageId, arch: actual.arch, gtk: actual.gtk, webkit: actual.webkit, tools: actual.tools}
}

export function approvedCiEnvironment(env) {
  assert.equal(env.GITHUB_ACTIONS, 'true', 'CI preparation is unavailable on ordinary hosts')
  assert.equal(env.GITHUB_REF, 'refs/heads/main', 'Only manually selected main is approved')
  assert.equal(env.BROWSEY_NATIVE_CI_EPHEMERAL, 'yes', 'Provisioned ephemeral image contract required')
  assert.ok(env.BROWSEY_NATIVE_CI_IMAGE_ID, 'Explicit approved image ID required')
  validatePath(env.RUNNER_TEMP, false)
  for (const name of ['DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'SSH_AUTH_SOCK'])
    assert.ok(!env[name], 'CI must not inherit a personal desktop or agent')
  const root = path.join(env.RUNNER_TEMP, 'ai_agent_testfolder')
  const config = {schema: 1, targets: {local: root}, rcloneConfig: null}
  makePlan(validateConfig(config), randomUUID()) // Refuse long Unix socket paths before writes.
  return config
}

export function redactedCiReport(report) {
  assert.equal(report.schema, 3)
  assert.equal(report.suite, 'smoke', 'CI accepts only local smoke evidence')
  assert.equal(report.evidence?.kind, 'REAL_NATIVE')
  assert.match(report.runId, /^[a-f0-9-]{36}$/)
  const required = ['input-local', 'copy-within-local', 'undo-copy-local', 'accessibility-local']
  assert.deepEqual(report.cases.map(item => item.id).sort(), [...required].sort())
  for (const item of report.cases) assert.deepEqual(item.parts.map(part => part.id).sort(), item.id === 'copy-within-local' ? ['directory', 'file'] : [])
  const stages = ['session-close', 'driver-exit', 'native-driver-exit', 'candidate-exit']
  const accepted = report.status === 'PASS' && report.cases.every(item => item.status === 'PASS' && item.parts.every(part => part.status === 'PASS'))
    && report.teardownDetails?.status === 'PASS' && JSON.stringify(report.teardownDetails.steps.map(step => step.stage)) === JSON.stringify(stages)
    && report.teardownDetails.steps.every(step => step.status === 'PASS')
    && report.desktop?.teardown === 'PASS' && report.retention?.status === 'PASS'
  return {schema: 1, evidence: 'REAL_NATIVE_LOCAL_SMOKE', runId: report.runId, status: accepted ? 'PASS' : 'BLOCKED',
    build: {commit: report.build?.commit ?? null, dirty: report.build?.dirty ?? null, sourceSha256: report.build?.sourceSha256 ?? null, binarySha256: report.build?.sha256 ?? null},
    harnessSha256: report.harnessSha256,
    host: {platform: report.host.platform, kernel: report.host.kernel, arch: report.host.arch, node: report.host.node, gtkWebkit: report.host.gtkWebkit, inputLayout: report.host.inputLayout},
    tools: {tauriDriverSha256: report.tools?.tauriDriverSha256 ?? null, webkitDriverSha256: report.tools?.webkitDriverSha256 ?? null}, cases: report.cases.map(item => ({id: item.id, status: item.status, acceptanceRows: acceptanceRows(item.id), parts: item.parts.map(part => ({id: part.id, status: part.status}))})),
    teardown: accepted ? 'PASS' : 'UNACCEPTED', retention: report.retention?.status ?? 'UNACCEPTED',
    notTested: ['Providers/devices/accounts', 'Installed production app', 'Other distributions/platforms', 'Performance budgets', 'Lifecycle transitions']}
}

async function toolchain(manifestPath, ci = false) {
  await noLinks(manifestPath, fs)
  const stat = await fs.lstat(manifestPath)
  assert.ok(stat.isFile() && stat.size < 65536 && stat.nlink === 1)
  if (ci) assert.ok(stat.uid === 0 && (stat.mode & 0o222) === 0, 'CI image manifest must be root-owned and read-only')
  const spec = JSON.parse(await fs.readFile(manifestPath, 'utf8'))
  const versions = (await exec('/usr/bin/pkg-config', ['--modversion', 'gtk+-3.0', 'webkit2gtk-4.1'])).stdout.trim().split('\n')
  const tools = Object.fromEntries(await Promise.all(toolNames.map(async name => [name,
    await fileSha256(name === 'bwrap' ? '/usr/bin/bwrap' : path.join(repo, 'target/native-tools/bin', name))])))
  return validateCiToolchain(spec, {arch: process.arch, gtk: versions[0], webkit: versions[1], tools})
}
async function main() {
  process.umask(0o077)
  const args = process.argv.slice(2)
  assert.ok(['--check', '--prepare', '--summary'].includes(args[0]))
  assert.ok(args.length === 1 || (args.length === 3 && args[1] === '--manifest' && args[0] === '--check'), 'Unsupported CI arguments')
  if (args[0] === '--check') {
    const result = await toolchain(path.resolve(args[2] ?? path.join(repo, 'frontend/e2e-native/ci-toolchain.example.json')))
    console.log(JSON.stringify({status: 'PASS', scope: 'Read-only toolchain metadata/hash match; not native or CI execution acceptance', ...result}))
    return
  }
  if (args[0] === '--summary') {
    const scope = await privateJson(path.join(repo, 'target/native-test/ci-scope.json'))
    const registry = await privateJson(path.join(repo, 'target/native-test/.retention/registry.json'))
    assert.equal(registry.runs.length, 1, 'CI workspace must contain exactly one registered run')
    const run = registry.runs[0], config = validateConfig(scope.config)
    const plan = makePlan(config, run.runId)
    const summary = redactedCiReport(await privateJson(path.join(plan.targets[0].run, 'report.json')))
    summary.imageId = scope.imageId
    await writePrivate(path.join(repo, 'target/native-test/ci-redacted.json'), JSON.stringify(summary, null, 2), {exclusive: true})
    if (summary.status !== 'PASS') process.exitCode = 1
    return
  }
  const config = approvedCiEnvironment(process.env)
  assert.ok(process.getuid() > 0, 'CI candidate must run unprivileged')
  const result = await toolchain('/opt/browsey-native/toolchain.json', true)
  assert.equal(result.imageId, process.env.BROWSEY_NATIVE_CI_IMAGE_ID, 'Provisioned/approved image IDs differ')
  await noLinks(process.env.RUNNER_TEMP, fs)
  privateStat(await fs.lstat(process.env.RUNNER_TEMP), true)
  const configPath = path.join(repo, 'frontend/e2e-native/config.local.json')
  await assert.rejects(fs.lstat(configPath), {code: 'ENOENT'}, 'Refuse existing local/credential configuration')
  await assert.rejects(fs.lstat(path.join(repo, 'target/native-test/.retention')), {code: 'ENOENT'}, 'Refuse reused CI registry/profile')
  await fs.mkdir(config.targets.local, {mode: 0o700}) // Exclusive approved ephemeral root.
  await fs.mkdir(path.join(repo, 'target/native-test'), {recursive: true, mode: 0o700})
  await fs.mkdir(path.join(repo, 'target/native-test/.retention'), {mode: 0o700})
  await writePrivate(configPath, JSON.stringify(config), {exclusive: true})
  await writePrivate(path.join(repo, 'target/native-test/ci-scope.json'), JSON.stringify({schema: 1, imageId: result.imageId, config}), {exclusive: true})
  console.log('Prepared one fresh approved ephemeral CI root; local smoke only, no account configuration.')
}
if (process.argv[1] === fileURLToPath(import.meta.url)) main().catch(error => {
  console.error(`Native CI contract stopped: ${error.message}`); process.exitCode = 1
})
