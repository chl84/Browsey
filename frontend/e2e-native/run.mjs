import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { randomUUID, createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import net from 'node:net'
import { validateConfig, makePlan, candidateEnvironment, child, kinds, noLinks } from './scope.mjs'
import { createLocalSession, Fixtures, regularFile } from './fixtures.mjs'
import { NativeUi } from './ui.mjs'
import { foundation } from './cases.mjs'

const exec = promisify(execFile)
const repo = fileURLToPath(new URL('../..', import.meta.url))
const candidate = path.join(repo, 'target/native-test/browsey')
const args = process.argv.slice(2)
const mode = args[0] ?? '--help'
const options = { config: path.join(repo, 'frontend/e2e-native/config.local.json'), targets: null, a11y: false }
for (let i = 1; i < args.length; i++) {
  if (args[i] === '--config') options.config = path.resolve(args[++i])
  else if (args[i] === '--targets') options.targets = args[++i]?.split(',')
  else if (args[i] === '--a11y') options.a11y = true
  else throw new Error('Unknown native runner argument')
}

async function tool(name, configured) {
  const dirs = ['/usr/bin', '/bin', path.join(repo, 'target/native-tools/bin')]
  const paths = configured ? [configured] : dirs.map(dir => path.join(dir, name))
  for (const raw of paths) {
    assert.ok(path.isAbsolute(raw), 'Explicit driver paths must be absolute')
    try { await fs.access(raw, fs.constants.X_OK); return raw } catch { /* Check next explicit tool location. */ }
  }
  throw new Error(`Missing ${name}; install the matching native test dependency (no app install is performed)`)
}

async function dependencies() {
  assert.equal(process.platform, 'linux', 'Native suite targets Linux')
  const driver = await tool('tauri-driver', process.env.BROWSEY_TAURI_DRIVER)
  const webkit = await tool('WebKitWebDriver', process.env.BROWSEY_WEBKIT_DRIVER)
  await noLinks(candidate, fs)
  const binary = await fs.open(candidate, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
  const build = JSON.parse((await regularFile(`${candidate}.json`)).text)
  assert.equal(build.feature, 'native-test')
  try {
    assert.ok((await binary.stat()).isFile())
    const hash = createHash('sha256')
    for await (const chunk of binary.createReadStream({ autoClose: false })) hash.update(chunk)
    assert.equal(build.sha256, hash.digest('hex'), 'Candidate changed after staging')
  } finally { await binary.close() }
  return { driver, webkit, build }
}

async function reservePorts() {
  // No attachment to an existing automation service. Bind both before creating
  // fixtures; tauri-driver then claims them immediately after they are released.
  const ports = [4444, 4445]
  for (const port of ports) {
    const server = net.createServer()
    await new Promise((resolve, reject) => {
      server.once('error', reject)
      server.listen(port, '127.0.0.1', resolve)
    })
    await new Promise(resolve => server.close(resolve))
  }
}

async function waitDriver(childProcess) {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    assert.equal(childProcess.exitCode, null, 'Owned tauri-driver exited during startup')
    try {
      const response = await fetch('http://127.0.0.1:4444/status', { signal: AbortSignal.timeout(1000) })
      if (response.ok) return
    } catch { /* Retry readiness only, never an operation. */ }
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  throw new Error('Owned tauri-driver did not become ready')
}

async function stopDriver(driver) {
  if (driver.exitCode !== null) return
  const exited = new Promise(resolve => driver.once('exit', resolve))
  driver.kill('SIGTERM')
  await Promise.race([exited, new Promise(resolve => setTimeout(resolve, 3000))])
  if (driver.exitCode === null) { driver.kill('SIGKILL'); await exited }
}

async function main() {
  if (mode === '--help') {
    console.log('Native suite: --plan | --check | --run [--config PATH] [--targets local,usb,...] [--a11y]\nBuild separately: bash scripts/dev/test-native-linux.sh --build\nOnly existing, explicitly approved ai_agent_testfolder roots are allowed.')
    return
  }
  assert.ok(['--plan', '--check', '--run'].includes(mode), 'Unknown native runner mode')
  let config = validateConfig(JSON.parse((await regularFile(options.config)).text))
  const configured = config.targets.map(target => target.kind)
  if (options.targets) {
    assert.ok(options.targets.includes('local') && options.targets.every(kind => configured.includes(kind)), 'Select configured kinds including local')
    config = { ...config, targets: config.targets.filter(target => options.targets.includes(target.kind)) }
    if (!options.targets.includes('cloud')) config.rcloneConfig = null
  }
  const plan = makePlan(config, randomUUID())
  if (mode === '--plan') {
    console.log(JSON.stringify({ ...plan, note: 'Plan only: no target files were inspected or changed' }, null, 2))
    return
  }
  if (mode === '--check') {
    const checks = []
    const check = async (name, action) => {
      try { await action(); checks.push({ name, status: 'PASS' }) }
      catch (error) { checks.push({ name, status: 'BLOCKED', reason: error.message }) }
    }
    await check('tauri-driver', () => tool('tauri-driver', process.env.BROWSEY_TAURI_DRIVER))
    await check('WebKitWebDriver', () => tool('WebKitWebDriver', process.env.BROWSEY_WEBKIT_DRIVER))
    await check('scoped candidate', () => fs.access(candidate, fs.constants.X_OK))
    for (const target of plan.targets.filter(target => target.kind !== 'cloud')) {
      await check(`${target.kind}: approved local path`, async () => {
        await noLinks(target.path, fs)
        assert.ok((await fs.lstat(target.path)).isDirectory(), 'Approved root is not an existing directory')
      })
    }
    if (config.rcloneConfig) await check('cloud: private test credentials', async () => {
      const credential = await regularFile(config.rcloneConfig)
      assert.equal(credential.stat.mode & 0o777, 0o600)
    })
    console.log(JSON.stringify({ mode, checks, note: 'Metadata only; no writes, connectivity test or native UI acceptance' }, null, 2))
    if (checks.some(check => check.status !== 'PASS')) process.exitCode = 1
    return
  }
  // Tool checks happen before target writes. No fallback to installed Browsey.
  const tools = await dependencies()
  await reservePorts()
  const profile = await createLocalSession(plan, config)
  const session = { runId: plan.runId, dataRoots: plan.targets.map(target => target.files), profile }
  const env = candidateEnvironment(process.env, profile, session)
  const fixture = new Fixtures(plan, env)
  const local = plan.targets.find(target => target.kind === 'local')
  const artifacts = child(local.run, 'artifacts')
  await fs.mkdir(artifacts, { mode: 0o700 })
  const reportPath = child(local.run, 'report.json')
  const harnessHash = createHash('sha256')
  for (const file of ['cases.mjs', 'fixtures.mjs', 'run.mjs', 'scope.mjs', 'ui.mjs']) {
    harnessHash.update(file).update(await fs.readFile(path.join(repo, 'frontend/e2e-native', file)))
  }
  harnessHash.update(await fs.readFile(path.join(repo, 'tests/support/native_fixture_a11y.py')))
  const report = { schema: 1, runId: plan.runId, started: new Date().toISOString(), build: tools.build,
    harnessSha256: harnessHash.digest('hex'),
    scope: 'Generated data in owned runs only; no installed app or personal settings',
    targets: Object.fromEntries(kinds.map(kind => [kind, plan.targets.some(target => target.kind === kind)
      ? 'NOT_RUN' : configured.includes(kind) ? 'DEFERRED' : 'NOT_CONFIGURED'])),
    cases: [], notTested: ['Address-entry keyboard layout', 'Native drag/drop', 'Mount/connect/unplug', 'Trash/format', 'Progress/cancellation with large files',
      'Archive/password/conflict handling', 'Other platforms/distributions', 'Watcher behavior (disabled in scoped candidate)'] }
  await fs.writeFile(reportPath, JSON.stringify(report, null, 2), { flag: 'wx', mode: 0o600 })
  let browser, driver
  const log = await fs.open(child(artifacts, 'driver.log'), 'wx', 0o600)
  const record = async (name, action) => {
    console.log(`Native case: ${name}`)
    const result = { name, status: 'RUNNING' }
    report.cases.push(result)
    try { await action(); result.status = 'PASS' }
    catch (error) { result.status = 'FAIL'; result.error = error.message; throw error }
    finally { await fs.writeFile(reportPath, JSON.stringify(report, null, 2), { mode: 0o600 }) }
  }
  try {
    for (const target of plan.targets.filter(target => target.kind === 'cloud')) await fixture.ensureCloudRoot(target)
    driver = spawn(tools.driver, ['--port', '4444', '--native-host', '127.0.0.1', '--native-port', '4445', '--native-driver', tools.webkit],
      { env, cwd: local.files, stdio: ['ignore', log.fd, log.fd] })
    let launchError
    driver.on('error', error => { launchError = error })
    await waitDriver(driver)
    if (launchError) throw launchError
    const { remote } = await import('webdriverio')
    browser = await remote({ hostname: '127.0.0.1', port: 4444, logLevel: 'silent', connectionRetryCount: 0,
      connectionRetryTimeout: 60_000, capabilities: { 'tauri:options': { application: candidate } } })
    const ui = new NativeUi(browser, session.dataRoots)
    const status = await ui.handshake(plan.runId)
    if (options.a11y) {
      // Desktop-control socket discovery is separate from the candidate's
      // private runtime. Still no personal HOME/config/credential inheritance.
      const a11yEnv = { ...env, XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
        HYPRLAND_INSTANCE_SIGNATURE: process.env.HYPRLAND_INSTANCE_SIGNATURE }
      ui.cancelDeleteAccessible = async () => {
        // Reuse the same case, with cancellation delivered through real AT-SPI.
        await exec('/usr/bin/python3', [path.join(repo, 'tests/support/native_fixture_a11y.py'),
          String(status.pid), candidate, `${profile}/data`, 'click', 'button|push button', 'Cancel'],
        { env: a11yEnv, cwd: local.files, timeout: 35_000, maxBuffer: 1024 * 1024 })
      }
      await record('AT-SPI: owned candidate accessibility tree', async () => {
        const result = await exec('/usr/bin/python3', [path.join(repo, 'tests/support/native_fixture_a11y.py'),
          String(status.pid), candidate, `${profile}/data`, 'snapshot', '', ''],
        { env: a11yEnv, cwd: local.files, timeout: 35_000, maxBuffer: 1024 * 1024 })
        const nodes = JSON.parse(result.stdout)
        assert.ok(nodes.some(node => node[1] === 'File list'), 'AT-SPI must expose the owned file list')
        await fs.writeFile(child(artifacts, 'accessibility.json'), result.stdout, { flag: 'wx', mode: 0o600 })
      })
    }
    await foundation(plan, fixture, ui, record)
    for (const target of plan.targets) report.targets[target.kind] = 'PASS'
    report.status = 'PASS'
  } catch (error) {
    report.status = report.cases.some(result => result.status === 'FAIL') ? 'FAIL' : 'BLOCKED'
    report.error = error.message
    if (browser) {
      try { await browser.saveScreenshot(child(artifacts, 'failure.png')) } catch { /* Only the owned window, never the desktop. */ }
    }
    throw error
  } finally {
    if (browser) {
      try { await browser.deleteSession() } catch { report.teardown = 'WebDriver session close failed; inspect owned process' }
    }
    if (driver) await stopDriver(driver)
    await log.close()
    report.finished = new Date().toISOString()
    await fs.writeFile(reportPath, JSON.stringify(report, null, 2), { mode: 0o600 })
    console.log(`Native report: ${reportPath}\n${report.status}; generated fixtures retained for inspection (no cleanup of existing files).`)
  }
}

main().catch(error => { console.error(`Native suite stopped: ${error.message}`); process.exitCode = 1 })
