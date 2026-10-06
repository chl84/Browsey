import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { randomUUID, createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import os from 'node:os'
import { validateConfig, makePlan, candidateEnvironment, child, noLinks } from './scope.mjs'
import { createLocalSession, Fixtures } from './fixtures.mjs'
import { NativeUi } from './ui.mjs'
import { foundation, foundationManifest } from './cases.mjs'
import { navigation, navigationManifest } from './navigation.mjs'
import { listing, listingManifest } from './listing.mjs'
import { selection, selectionManifest } from './selection.mjs'
import { creation, creationManifest } from './creation.mjs'
import { names, namesManifest } from './names.mjs'
import { limits, limitsManifest } from './limits.mjs'
import { contents, contentsManifest } from './contents.mjs'
import { trees, treesManifest } from './trees.mjs'
import { links, linksManifest } from './links.mjs'
import { usb, usbManifest } from './providers.mjs'
import { linkPlan } from './link-policy.mjs'
import { editing, editingManifest } from './editing.mjs'
import { overwrites, overwriteManifest, overwriteProbes } from './overwrite.mjs'
import { moves, moveManifest, moveProbes, moveFaults } from './moves.mjs'
import { access, accessManifest } from './access.mjs'
import { ioFaults, ioFaultManifest, ioFaultProbes } from './iofaults.mjs'
import { races, raceManifest, raceProbes } from './races.mjs'
import { interruption, interruptionManifest, interruptionProbes } from './interruption.mjs'
import { ownedRestart } from './restart.mjs'
import { cancellations, cancellationManifest, cancellationProbes } from './cancellation.mjs'
import { transfers, transferManifest } from './transfers.mjs'
import { batches, batchManifest, batchFaults } from './batch.mjs'
import { guards, guardManifest } from './guards.mjs'
import { conflicts, conflictManifest } from './conflicts.mjs'
import { routeEvidence } from './routing.mjs'
import { progress, progressManifest, progressProbes } from './progress.mjs'

import { verifyCandidate, fileSha256 } from './candidate.mjs'
import { createReport, recordSetup, recordCase, finishReport, summarizeProviders } from './report.mjs'
import { privateJson, assertPrivateFile, writePrivate, inspectTree, processStamp, retentionPolicy } from './privacy.mjs'
import { RetentionStore } from './retention.mjs'
import { bounded, trackChild, checkPorts, waitDriver, ownedNativeDriverPid, assertDriverAlive,
  captureCandidate, assertCandidateAlive, candidateState, stopCandidate, teardown, applyTeardown } from './lifecycle.mjs'

const exec = promisify(execFile)
const suites = { foundation: { run: foundation, manifest: foundationManifest },
  navigation: { run: navigation, manifest: navigationManifest }, listing: { run: listing, manifest: listingManifest },
  selection: { run: selection, manifest: selectionManifest }, creation: { run: creation, manifest: creationManifest } }
for (const group of ['editing', 'fileops', 'rename', 'properties', 'history']) suites[group] = {
  run: (plan, fixture, ui, record) => editing(plan, fixture, ui, record, group),
  manifest: plan => editingManifest(plan, group),
}
suites['transfers-within'] = { run: transfers, manifest: transferManifest }
suites['transfers-hub'] = { run: (plan, fixture, ui, record) => transfers(plan, fixture, ui, record, 'hub'),
  manifest: plan => transferManifest(plan, 'hub') }
suites['transfers-pairs'] = { run: (plan, fixture, ui, record) => transfers(plan, fixture, ui, record, 'pairs'),
  manifest: plan => transferManifest(plan, 'pairs') }
suites['guards-aliases-mobile'] = { run: (plan, fixture, ui, record) => guards(plan, fixture, ui, record, 'remaining'),
  manifest: plan => guardManifest(plan, 'remaining') }
suites.batches = { run: batches, manifest: batchManifest }
suites.guards = { run: guards, manifest: guardManifest }
suites.conflicts = { run: conflicts, manifest: conflictManifest }
suites.progress = { run: progress, manifest: progressManifest }
suites.cancellation = { run: cancellations, manifest: cancellationManifest }
suites.overwrite = { run: overwrites, manifest: overwriteManifest }
suites.moves = { run: moves, manifest: moveManifest }
suites.access = { run: access, manifest: accessManifest }
suites.iofaults = { run: ioFaults, manifest: ioFaultManifest }
suites.races = { run: races, manifest: raceManifest }
suites.interruption = { run: interruption, manifest: interruptionManifest }
suites.names = { run: names, manifest: namesManifest }
suites.limits = { run: limits, manifest: limitsManifest }
suites.contents = { run: contents, manifest: contentsManifest }
suites.trees = { run: trees, manifest: treesManifest }
suites.links = { run: links, manifest: linksManifest }
suites.usb = { run: usb, manifest: usbManifest }
suites['usb-access'] = { run: (plan,fixture,ui,record)=>access(plan,fixture,ui,record,'usb'), manifest: plan=>{usbManifest(plan);return accessManifest(plan,'usb')} }
const repo = fileURLToPath(new URL('../..', import.meta.url))
const candidate = path.join(repo, 'target/native-test/browsey')
// Inherited by the scoped app/drivers only; no desktop/global permission change.
process.umask(0o077)
const args = process.argv.slice(2)
const mode = args[0] ?? '--help'
const options = { config: path.join(repo, 'frontend/e2e-native/config.local.json'), targets: null, suite: 'foundation', fullscreen: false, a11y: false, fault: null, reportFault: null }
const reportFaults = ['setup-failure', 'partial-transfer']
const faults = ['driver-exit', 'candidate-exit', 'session-close', 'session-timeout', 'case-timeout']
for (let i = 1; i < args.length; i++) {
  if (args[i] === '--config') options.config = path.resolve(args[++i])
  else if (args[i] === '--targets') options.targets = args[++i]?.split(',')
  else if (args[i] === '--suite') {
    options.suite = args[++i]
    assert.ok(Object.hasOwn(suites, options.suite), 'Expected a supported native suite')
  }
  else if (args[i] === '--a11y') options.a11y = true
  else if (args[i] === '--fullscreen') options.fullscreen = true
  else if (args[i] === '--lifecycle-fault') {
    options.fault = args[++i]
    assert.ok(faults.includes(options.fault), 'Expected a supported lifecycle fault')
  }
  else if (args[i] === '--report-fault') {
    options.reportFault = args[++i]
    assert.ok(reportFaults.includes(options.reportFault), 'Expected a supported report fault')
  }
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
  const build = await verifyCandidate(repo, candidate)
  return { driver, webkit, build }
}

async function main() {
  if (mode === '--help') {
    console.log('Native suite: --plan | --check | --run [--config PATH] [--targets local,usb,...] [--suite ' + Object.keys(suites).join('|') + '] [--a11y] [--fullscreen]\nFullscreen requires explicit maintainer approval; only the captured candidate window is targeted.\nLifecycle faults: --run --targets local --lifecycle-fault ' + faults.join('|') + '\nReport faults: --run --targets local --report-fault ' + reportFaults.join('|') + '\nBuild separately: bash scripts/dev/test-native-linux.sh --build\nOnly existing, explicitly approved ai_agent_testfolder roots are allowed.')
    return
  }
  assert.ok(['--plan', '--check', '--run'].includes(mode), 'Unknown native runner mode')
  assert.ok(!(options.fault || options.reportFault) || options.suite === 'foundation', 'Fault injection requires the foundation suite')
  if (options.fault) assert.ok(mode === '--run' && faults.includes(options.fault)
    && options.targets?.length === 1 && options.targets[0] === 'local' && !options.a11y,
  'Lifecycle faults require --run --targets local without accessibility or other providers')
  if (options.reportFault) assert.ok(mode === '--run' && !options.fault
    && options.targets?.length === 1 && options.targets[0] === 'local' && !options.a11y,
  'Report faults require --run --targets local without accessibility, lifecycle faults or other providers')
  let config = validateConfig(await privateJson(options.config))
  const configured = config.targets.map(target => target.kind)
  if (options.targets) {
    assert.ok(options.targets.includes('local') && options.targets.every(kind => configured.includes(kind)), 'Select configured kinds including local')
    config = { ...config, targets: config.targets.filter(target => options.targets.includes(target.kind)) }
    if (!options.targets.includes('cloud')) config.rcloneConfig = null
  }
  if (options.suite === 'interruption') assert.deepEqual(config.targets.map(t => t.kind), ['local'],
    'Owned process interruption requires --targets local without provider subprocesses')
  if (options.suite === 'links') assert.deepEqual(config.targets.map(t => t.kind), ['local'],
    'Owned leaf-link acceptance requires --targets local; other providers are deferred')
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
    await check('scoped candidate identity', async () => {
      await fs.access(candidate, fs.constants.X_OK)
      await verifyCandidate(repo, candidate)
    })
    for (const target of plan.targets.filter(target => target.kind !== 'cloud')) {
      await check(`${target.kind}: approved local path`, async () => {
        await noLinks(target.path, fs)
        assert.ok((await fs.lstat(target.path)).isDirectory(), 'Approved root is not an existing directory')
      })
    }
    if (config.rcloneConfig) await check('cloud: private test credentials', async () => {
      await assertPrivateFile(config.rcloneConfig)
    })
    console.log(JSON.stringify({ mode, checks, note: 'Metadata only; no writes, connectivity test or native UI acceptance' }, null, 2))
    if (checks.some(check => check.status !== 'PASS')) process.exitCode = 1
    return
  }
  const manifest = options.fault ? [{ id: 'lifecycle-owned-window', name: 'Native lifecycle: owned window identity', providers: ['local'] }]
    : suites[options.suite].manifest(plan)
  if (options.a11y) manifest.push({ id: 'accessibility-local', name: 'AT-SPI: owned candidate accessibility tree', providers: ['local'] })
  const report = createReport(plan, configured, manifest)
  const local = plan.targets.find(target => target.kind === 'local')
  const artifacts = child(local.run, 'artifacts')
  const reportPath = child(local.run, 'report.json')
  Object.assign(report, { suite: options.suite, scope: options.fault ? 'Injected lifecycle fault in owned local session; no UI file operations'
    : 'Generated data in owned runs only; no installed app or personal settings',
  host: { platform: process.platform, kernel: os.release(), arch: process.arch, node: process.version,
    inputLayout: process.env.BROWSEY_NATIVE_INPUT_LAYOUT ?? 'NOT_RECORDED' },
  injectedLifecycleFault: options.fault, injectedReportFault: options.reportFault,
  notTested: ['Other keyboard layouts', 'Non-BMP Unicode text entry (native WebDriver drops emoji)',
    'Native drag/drop', 'Mount/connect/unplug', 'Trash/format', 'Progress/cancellation with large files',
    'Archive/password and broader transfer-conflict handling', 'Other platforms/distributions', 'Watcher behavior (disabled in scoped candidate)'] })
  if (options.suite === 'links') report.notTested.push('Link behavior on USB/network/cloud/mobile', 'Directory symlinks and outside referents (not authorized)')
  if (options.fault) report.notTested.push('All UI file-operation acceptance (lifecycle fault scope)')
  if (options.suite === 'selection') report.notTested.push('Large-list virtualization on USB/network/cloud/mobile (local representative only)',
    'Backend invocation receipts and repeated copy/paste submission (NT1-6)')
  let browser, driver, nativeDriverOwner, candidateOwner, log, reportOwned = false, reservation, activeCreated = false
  let tools, profile, session, env, fixture
  const retention = new RetentionStore(path.join(repo, 'target/native-test/.retention'))
  const activePath = child(local.run, 'active.json')
  const persist = async () => {
    summarizeProviders(report)
    if (reportOwned) await writePrivate(reportPath, JSON.stringify(report, null, 2))
  }
  const setup = (metadata, action) => recordSetup(report, metadata, async () => {
    if (options.reportFault === 'setup-failure' && metadata.id === 'private-profile') {
      throw Object.assign(new Error('Injected private-profile setup failure'), { failureKind: 'INJECTED_SETUP' })
    }
    return action()
  }, persist)
  const shared = id => ({ id, providers: plan.targets.map(target => target.kind) })
  const owned = async () => {
    assertDriverAlive(driver)
    await assertCandidateAlive(nativeDriverOwner)
    await assertCandidateAlive(candidateOwner)
  }
  const record = async (name, action, metadata) => {
    console.log(`Native case: ${name}`)
    return recordCase(report, metadata, action, { before: owned, after: owned, persist,
      resolveFailure: async error => { try { await owned(); return error } catch (observed) { return observed } } })
  }
  try {
    // Preflight still precedes writes. Failures before local ownership print a
    // structured report; failures afterward retain it in the owned local run.
    await setup(shared('harness-identity'), async () => {
      const harnessHash = createHash('sha256')
      for (const file of ['providers.mjs', 'links.mjs', 'link-policy.mjs', 'resources.mjs', 'trees.mjs', 'byte-tree.mjs', 'contents.mjs', 'limits.mjs', 'names.mjs', 'candidate.mjs', 'interruption.mjs', 'races.mjs', 'iofaults.mjs', 'access.mjs', 'moves.mjs', 'overwrite.mjs', 'recovery.mjs', 'cancellation.mjs', 'progress.mjs', 'batch.mjs', 'conflicts.mjs', 'guards.mjs', 'cases.mjs', 'creation.mjs', 'editing.mjs', 'restart.mjs', 'routing.mjs', 'transfers.mjs', 'fixtures.mjs', 'lifecycle.mjs', 'listing.mjs', 'navigation.mjs', 'selection.mjs', 'privacy.mjs', 'report.mjs', 'retention.mjs', 'run.mjs', 'scope.mjs', 'ui.mjs']) {
        harnessHash.update(file).update(await fs.readFile(path.join(repo, 'frontend/e2e-native', file)))
      }
      harnessHash.update(await fs.readFile(path.join(repo, 'tests/support/native_fixture_a11y.py')))
      report.harnessSha256 = harnessHash.digest('hex')
    })
    tools = await setup(shared('dependencies'), dependencies)
    report.build = tools.build
    await setup(shared('driver-ports'), checkPorts)
    profile = await createLocalSession(plan, config, { step: setup,
      beforeOwned: () => setup(shared('retention-budget'), async () => { reservation = await retention.reserve(plan); return reservation }),
      onOwned: async () => {
      await writePrivate(reportPath, JSON.stringify(report, null, 2), { exclusive: true })
      reportOwned = true
      await writePrivate(activePath, JSON.stringify({ runId: plan.runId, nonce: reservation.nonce }), { exclusive: true })
      activeCreated = true
    } })
    session = { runId: plan.runId, dataRoots: plan.targets.map(target => target.files), profile,
      ...(options.suite === 'links' ? { links: linkPlan(plan) } : {}),
      ...(options.suite === 'progress' ? { probes: progressProbes(plan) } : options.suite === 'cancellation' ? { probes: cancellationProbes(plan) } : options.suite === 'overwrite' ? { probes: overwriteProbes(plan) } : options.suite === 'moves' ? { probes: moveProbes(plan) } : options.suite === 'iofaults' ? { probes: ioFaultProbes(plan) } : options.suite === 'races' ? { probes: raceProbes(plan) } : options.suite === 'interruption' ? { probes: interruptionProbes(plan) } : {}),
      ...(options.suite === 'batches' ? { faults: batchFaults(plan) } : options.suite === 'moves' ? { faults: moveFaults(plan) } : {}) }
    if (session.faults) report.faults = session.faults.map(fault => ({ id: fault.id,
      kind: fault.source ? 'owned-source-dispatch' : 'owned-list-refresh', status: 'NOT_RUN', uses: 0 }))
    env = candidateEnvironment(process.env, profile, session)
    if (options.suite === 'transfers-pairs') env.RUST_LOG = 'browsey=info'
    fixture = new Fixtures(plan, env)
    if (options.reportFault === 'partial-transfer') {
      const read = fixture.read.bind(fixture)
      fixture.read = async raw => {
        if (raw === `${local.files}/copy-within-local-target/tree/nested.txt`) {
          throw Object.assign(new Error('Injected directory verification I/O failure; no retry'), { failureKind: 'FIXTURE_IO' })
        }
        return read(raw)
      }
    }
    await setup(shared('artifacts'), async () => {
      await fs.mkdir(artifacts, { mode: 0o700 })
      log = await fs.open(child(artifacts, 'driver.log'), 'wx', 0o600)
    })
    await setup(shared('host-tool-evidence'), async () => {
      report.host.gtkWebkit = (await exec('/usr/bin/pkg-config', ['--modversion', 'gtk+-3.0', 'webkit2gtk-4.1'])).stdout.trim().split('\n')
      report.tools = { tauriDriverSha256: await fileSha256(await fs.realpath(tools.driver)),
        webkitDriverSha256: await fileSha256(await fs.realpath(tools.webkit)) }
    })
    for (const target of plan.targets.filter(target => target.kind === 'cloud')) {
      await setup({ id: 'owned-run-cloud', providers: ['cloud'] }, () => fixture.ensureCloudRoot(target))
    }
    const startDriver = async () => {
      driver = spawn(tools.driver, ['--port', '4444', '--native-host', '127.0.0.1', '--native-port', '4445', '--native-driver', tools.webkit],
        { env, cwd: local.files, stdio: ['ignore', log.fd, log.fd] })
      trackChild(driver)
      report.driverIdentity = { pid: driver.pid ?? null }
      assertDriverAlive(driver)
      report.driverIdentity.startTime = (await processStamp(driver.pid)).start
      if (options.fault === 'driver-exit') driver.kill('SIGTERM')
      const webkitExecutable = await fs.realpath(tools.webkit)
      await waitDriver(driver, { listening: async () => {
        const pid = await ownedNativeDriverPid(driver, webkitExecutable)
        if (!pid) return false
        nativeDriverOwner = await captureCandidate(pid, { executable: webkitExecutable, dataHome: `${profile}/data`,
          runId: plan.runId, role: 'native-driver' })
        report.driverIdentity.nativePid = nativeDriverOwner.pid
        report.driverIdentity.nativeStartTime = nativeDriverOwner.start
        return true
      } })
    }
    const startSession = async () => {
      const { remote } = await import('webdriverio')
      browser = await remote({ hostname: '127.0.0.1', port: 4444, logLevel: 'silent', connectionRetryCount: 0,
        connectionRetryTimeout: 60_000, capabilities: { 'tauri:options': { application: candidate } } })
    }
    await setup(shared('driver-startup'), startDriver)
    await setup(shared('webdriver-session'), startSession)
    const ui = new NativeUi(browser, session.dataRoots)
    if (['progress', 'cancellation', 'overwrite', 'moves', 'access', 'iofaults', 'races', 'interruption'].includes(options.suite)) ui.waitTimeout = 180_000
    if (options.suite === 'transfers-pairs') ui.transferEvidence = routeEvidence(`${profile}/data/browsey/logs/browsey.log`)
    const captureIdentity = async () => {
      ui.browser = browser
      const status = await ui.handshake(plan.runId)
      candidateOwner = await captureCandidate(status.pid, { executable: candidate, dataHome: `${profile}/data`, runId: plan.runId })
      report.identity = { pid: status.pid, startTime: candidateOwner.start, runId: status.runId, scope: status.scope, watcher: status.watcher }
      return status
    }
    let status = await setup(shared('candidate-identity'), captureIdentity)
    const a11yEnv = { ...env, XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
      HYPRLAND_INSTANCE_SIGNATURE: process.env.HYPRLAND_INSTANCE_SIGNATURE }
    const fullscreen = async () => {
      await owned()
      const result = await exec('/usr/bin/python3', [path.join(repo, 'tests/support/native_fixture_a11y.py'),
        String(status.pid), candidate, `${profile}/data`, 'fullscreen', '', ''],
      { env: a11yEnv, cwd: local.files, timeout: 15_000, maxBuffer: 1024 * 1024 })
      report.window = JSON.parse(result.stdout)
      await owned()
    }
    if (options.fullscreen) await setup(shared('owned-window-fullscreen'), fullscreen)
    if (['editing', 'history', 'interruption'].includes(options.suite)) {
      report.restarts = []
      let interrupted = false
      ui.restart = ownedRestart({ restarts: report.restarts, persist,
        stop: async () => {
          if (interrupted) assert.equal(await candidateState(candidateOwner), 'exited',
            'Confirm the deliberately interrupted candidate is gone before restart teardown')
          else await owned()
          const identity = { candidate: report.identity, driver: report.driverIdentity }
          const result = await teardown({ browser, driver, nativeDriver: nativeDriverOwner, candidate: candidateOwner })
          browser = undefined // Never submit a second deleteSession for this session.
          return { ...identity, teardown: result }
        },
        start: async () => {
          driver = undefined; nativeDriverOwner = undefined; candidateOwner = undefined
          await checkPorts(); await verifyCandidate(repo, candidate)
          await startDriver(); await startSession(); status = await captureIdentity()
          await owned(); if (options.fullscreen) await fullscreen()
          return { candidate: report.identity, driver: report.driverIdentity }
        },
      })
      if (options.suite === 'interruption') ui.interruptAndRestart = async () => {
        await owned()
        assert.equal(interrupted, false, 'Only one owned process interruption is allowed')
        report.interruption = { candidate: report.identity, signal: 'SIGKILL', status: 'RUNNING' }
        await persist()
        await stopCandidate(candidateOwner, { signal: pid => process.kill(pid, 'SIGKILL') })
        interrupted = true
        report.interruption.status = 'EXIT_CONFIRMED'
        await persist()
        await ui.restart()
      }
    }
    if (options.fault) {
      await record('Native lifecycle: owned window identity', async () => {
        if (options.fault === 'candidate-exit') await stopCandidate(candidateOwner, { signal: pid => process.kill(pid, 'SIGKILL') })
        if (options.fault === 'case-timeout') await bounded(() => new Promise(() => {}), 100,
          'CASE_TIMEOUT', 'Injected read-only lifecycle wait timed out')
      }, { id: 'lifecycle-owned-window', providers: ['local'] })
    }
    if (options.a11y) {
      // Desktop-control socket discovery is separate from the candidate's
      // private runtime. Still no personal HOME/config/credential inheritance.
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
      }, { id: 'accessibility-local', providers: ['local'] })
    }
    if (!options.fault) await suites[options.suite].run(plan, fixture, ui, record)
    if (session.faults) {
      const status = await ui.handshake(plan.runId)
      report.faults = report.faults.map(fault => ({ ...fault, status: 'USED',
        uses: status.faults.find(observed => observed.id === fault.id)?.uses ?? 0 }))
      assert.ok(report.faults.every(fault => fault.uses === 1), 'Every declared native fault must be consumed exactly once')
    }
    assertDriverAlive(driver)
    await assertCandidateAlive(nativeDriverOwner)
    await assertCandidateAlive(candidateOwner)
    finishReport(report)
  } catch (error) {
    finishReport(report, error)
    if (browser && candidateOwner) {
      try {
        assertDriverAlive(driver)
        if (await candidateState(candidateOwner) === 'alive') await bounded(() => browser.saveScreenshot(child(artifacts, 'failure.png')),
          5000, 'SCREENSHOT_TIMEOUT', 'Owned-window failure screenshot timed out')
      } catch { /* Only the still-owned window; teardown must proceed even if capture fails. */ }
    }
    throw error
  } finally {
    const closingBrowser = browser && ['session-close', 'session-timeout'].includes(options.fault) ? {
      deleteSession: () => options.fault === 'session-timeout' ? new Promise(() => {}) : Promise.reject(new Error('Injected session closure failure')),
    } : browser
    applyTeardown(report, await teardown({ browser: closingBrowser, driver, nativeDriver: nativeDriverOwner, candidate: candidateOwner }))
    if (report.status !== 'PASS') process.exitCode = 1
    if (report.teardownDetails.status !== 'PASS' && !report.failureKind) {
      report.failureKind = report.teardownDetails.steps.find(step => step.status !== 'PASS').failureKind
      report.failureOrigin = 'harness'
    }
    await log?.close()
    report.finished = new Date().toISOString()
    await persist()
    if (reportOwned && reservation) {
      try {
        const audit = await inspectTree(local.run)
        report.retention = { status: 'PASS', private: true, bytes: audit.bytes, entries: audit.entries.length,
          policy: retentionPolicy, scope: 'This registered local run; failed/multi-provider recovery retained' }
        await persist()
        if (activeCreated && report.teardownDetails.status === 'PASS') {
          const active = await privateJson(activePath)
          assert.equal(active.runId, plan.runId)
          assert.equal(active.nonce, reservation.nonce)
          await fs.unlink(activePath)
        }
        await retention.complete(plan.runId, await inspectTree(local.run), report.status)
      } catch (error) {
        report.retention = { status: 'BLOCKED', error: error.message }
        if (report.status === 'PASS') report.status = 'BLOCKED'
        report.failureKind ??= 'PRIVACY_OR_RETENTION'
        report.failureOrigin ??= 'harness'
        process.exitCode = 1
        await persist()
        await retention.complete(plan.runId, null, report.status)
      }
    } else if (reservation) await retention.complete(plan.runId, null, 'BLOCKED')
    if (reportOwned) console.log(`Native report: ${reportPath}\n${report.status}; generated fixtures retained for inspection (no cleanup of existing files).`)
    else console.log(`Native preflight report: ${JSON.stringify(report)}\n${report.status}; no owned report path was established.`)
  }
}

main().catch(error => { console.error(`Native suite stopped: ${error.message}`); process.exitCode = 1 })
