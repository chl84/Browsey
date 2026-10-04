import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
import { once } from 'node:events'
import { fileURLToPath } from 'node:url'
import { configuration, processorArgs, processReport } from './process-report.mjs'
import { latestStatus, readLedger, reportId, writeLedger, validateOutcome, archiveReport } from './report-ledger.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))

function git(source, ...args) {
  const result = spawnSync('git', ['-C', source, ...args], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  return result.stdout.trim()
}

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'browsey-report-test-'))
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))
  const source = path.join(root, 'source with spaces')
  fs.mkdirSync(source)
  git(source, 'init', '-b', 'main')
  git(source, 'config', 'user.name', 'Test')
  git(source, 'config', 'user.email', 'test@example.invalid')
  fs.writeFileSync(path.join(source, 'file.txt'), 'original')
  fs.writeFileSync(path.join(source, '.gitignore'), '/docs/maintenance/agent-reports/\n')
  git(source, 'add', '.')
  git(source, '-c', 'commit.gpgsign=false', 'commit', '-m', 'Fixture')
  const reports = path.join(source, 'docs/maintenance/agent-reports')
  fs.mkdirSync(reports, { recursive: true })
  fs.writeFileSync(path.join(reports, 'latest.txt'), 'Review: improve tooltip behavior.\n')
  const state = path.join(root, 'state')
  const logs = path.join(state, 'logs')
  const config = { source, reports, state, logs, branch: 'main', maxMinutes: 1 }
  const configFile = path.join(root, 'config.json')
  fs.writeFileSync(configFile, JSON.stringify({ ...config, repository: source, state, reports }))
  return { root, config, configFile }
}

function outcome(id, status = 'implemented') {
  return {
    reportId: id, summary: 'Behandlet siste rapport.',
    findings: [{ title: 'Tooltip behavior', status, reason: 'Verified current implementation.', files: ['file.txt'],
      verification: status === 'implemented' ? [{ command: 'node --test fixture.test.mjs', result: 'passed', details: '1 test passed' }] : [], remainingRisks: [] }],
  }
}

function fakeCodex(f, body = '', result = outcome(reportId(fs.readFileSync(path.join(f.config.reports, 'latest.txt'), 'utf8'))), events = true) {
  const executable = path.join(f.root, 'fake-codex')
  const event = { type: 'item.completed', item: { type: 'command_execution', command: '/bin/bash -lc "node --test fixture.test.mjs"', exit_code: 0 } }
  fs.writeFileSync(executable, `#!${process.execPath}\nconst fs=require('fs'); const a=process.argv; const output=a[a.indexOf('--output-last-message')+1];\n${body}\n${events ? `console.log(${JSON.stringify(JSON.stringify(event))});` : ''}\nfs.writeFileSync(output, ${JSON.stringify(JSON.stringify(result))});process.stdin.resume();\n`, { mode: 0o700 })
  f.config.codex = executable
}

function completed(f) {
  const latest = latestStatus(f.config.reports)
  writeLedger(f.config.reports, { version: 1, reports: [{ id: latest.id, sourceReport: 'latest.txt', attempts: [{ state: 'completed', findings: [] }] }] })
}

test('hash identifies report contents, including distinct report timestamps', () => {
  assert.equal(reportId('same text'), reportId('same text'))
  assert.notEqual(reportId('same text'), reportId('same text\n'))
  assert.notEqual(reportId('Review 2026-10-04\nSuggestion'), reportId('Review 2026-10-11\nSuggestion'))
})

test('missing handling log means pending; invalid log fails closed', t => {
  const f = fixture(t)
  assert.equal(latestStatus(f.config.reports).state, 'pending')
  fs.writeFileSync(path.join(f.config.reports, 'handling-log.json'), '{broken')
  assert.throws(() => latestStatus(f.config.reports))
})

test('duplicate report identities and unknown attempt states are rejected', t => {
  const f = fixture(t)
  completed(f)
  const ledger = readLedger(f.config.reports)
  ledger.reports.push(ledger.reports[0])
  writeLedger(f.config.reports, ledger)
  assert.throws(() => readLedger(f.config.reports), /duplicate/)
  ledger.reports.pop()
  ledger.reports[0].attempts[0].state = 'invented'
  writeLedger(f.config.reports, ledger)
  assert.throws(() => readLedger(f.config.reports), /attempt/)
})

test('symlink latest, ledger and report-directory ancestors are refused', t => {
  const f = fixture(t)
  const latest = path.join(f.config.reports, 'latest.txt')
  fs.unlinkSync(latest)
  fs.symlinkSync(path.join(f.config.source, 'file.txt'), latest)
  assert.throws(() => latestStatus(f.config.reports), /regular/)
  fs.unlinkSync(latest)
  fs.writeFileSync(latest, 'Report')
  fs.symlinkSync(path.join(f.config.source, 'file.txt'), path.join(f.config.reports, 'handling-log.json'))
  assert.throws(() => latestStatus(f.config.reports), /regular/)
})

test('report archives are stable and never overwrite mismatched contents', t => {
  const f = fixture(t)
  const archive = archiveReport(f.config.reports, 'Frozen report')
  assert.equal(archiveReport(f.config.reports, 'Frozen report'), archive)
  fs.writeFileSync(path.join(f.config.reports, archive), 'Changed externally')
  assert.throws(() => archiveReport(f.config.reports, 'Frozen report'), /identity mismatch/)
  assert.equal(fs.readFileSync(path.join(f.config.reports, archive), 'utf8'), 'Changed externally')
})

test('already-handled report skips model even when source is dirty', async t => {
  const f = fixture(t)
  completed(f)
  f.config.codex = '/does-not-exist'
  fs.writeFileSync(path.join(f.config.source, 'file.txt'), 'User work')
  assert.equal(await processReport(f.config), 'handled')
  assert.equal(fs.existsSync(f.config.logs), false)
  assert.equal(fs.readFileSync(path.join(f.config.source, 'file.txt'), 'utf8'), 'User work')
})

test('dirty source and another branch block unhandled processing without ledger changes', async t => {
  const f = fixture(t)
  fs.writeFileSync(path.join(f.config.source, 'file.txt'), 'User work')
  await assert.rejects(processReport(f.config), /clean/)
  assert.equal(fs.existsSync(path.join(f.config.reports, 'handling-log.json')), false)
  fs.writeFileSync(path.join(f.config.source, 'file.txt'), 'original')
  git(f.config.source, 'checkout', '-b', 'other')
  await assert.rejects(processReport(f.config), /clean/)
})

test('configuration refuses another checkout and symlink state', t => {
  const f = fixture(t)
  const env = { BROWSEY_MAINTENANCE_CONFIG: f.configFile }
  assert.equal(configuration(f.config.source, env).source, f.config.source)
  const config = JSON.parse(fs.readFileSync(f.configFile))
  config.repository = f.root
  fs.writeFileSync(f.configFile, JSON.stringify(config))
  assert.throws(() => configuration(f.config.source, env), /original repository/)
  config.repository = f.config.source
  fs.writeFileSync(f.configFile, JSON.stringify(config))
  fs.symlinkSync(f.root, f.config.state)
  assert.throws(() => configuration(f.config.source, env), /Symlink/)
})

test('processor uses workspace-write without inherited bypass, integrations or network', () => {
  const args = processorArgs({ source: '/source' }, '/result')
  assert.equal(args[args.indexOf('--sandbox') + 1], 'workspace-write')
  assert.ok(args.includes('--ignore-user-config'))
  assert.ok(args.includes('--ignore-rules'))
  assert.ok(args.includes('--ephemeral'))
  assert.ok(args.includes('approval_policy="never"'))
  assert.ok(args.includes('sandbox_workspace_write.network_access=false'))
  assert.ok(args.includes('--output-schema'))
  assert.ok(args.includes('features.plugins=false'))
  assert.ok(!args.some(arg => /bypass|danger-full-access|--add-dir|--worktree/.test(arg)))
})

test('wrong identity, malformed dispositions and invented verification are rejected', () => {
  const id = reportId('Report')
  const commands = [{ command: '/bin/bash -lc "node --test fixture.test.mjs"', exitCode: 0 }]
  assert.throws(() => validateOutcome(outcome(id), reportId('Other report'), commands), /identity/)
  assert.throws(() => validateOutcome(outcome(id), id, []), /evidence/)
  const value = outcome(id)
  value.findings[0].status = 'unknown'
  assert.throws(() => validateOutcome(value, id, commands), /disposition/)
  value.findings[0].status = 'implemented'
  value.findings[0].verification = []
  assert.throws(() => validateOutcome(value, id, commands), /verification/)
})

test('failed checks cannot certify an implemented finding', () => {
  const id = reportId('Report')
  const value = outcome(id)
  value.findings[0].verification.push({ command: 'other-test', result: 'failed', details: 'Failure' })
  assert.throws(() => validateOutcome(value, id, [{ command: 'node --test fixture.test.mjs', exitCode: 0 }]), /failed checks/)
})

test('intentional pre-fix failure followed by successful rerun can certify a fix', () => {
  const id = reportId('Report')
  const value = outcome(id)
  value.findings[0].verification.unshift({ command: 'node --test fixture.test.mjs', result: 'expected-failure', details: 'Regression failed before fix' })
  const commands = [{ command: 'node --test fixture.test.mjs', exitCode: 1 }, { command: 'node --test fixture.test.mjs', exitCode: 0 }]
  assert.equal(validateOutcome(value, id, commands), value)
  assert.throws(() => validateOutcome(value, id, commands.slice(1)), /pre-fix/)
})

test('an earlier passing command cannot hide a later failure', () => {
  const id = reportId('Report')
  assert.throws(() => validateOutcome(outcome(id), id, [
    { command: 'node --test fixture.test.mjs', exitCode: 0 },
    { command: 'node --test fixture.test.mjs', exitCode: 1 },
  ]), /evidence/)
})

test('successful processing records verified findings and changes without committing', async t => {
  const f = fixture(t)
  const head = git(f.config.source, 'rev-parse', 'HEAD')
  fakeCodex(f, "fs.writeFileSync('file.txt','Implemented change');")
  assert.equal(await processReport(f.config), 'completed')
  assert.equal(latestStatus(f.config.reports).state, 'handled')
  const record = readLedger(f.config.reports).reports[0]
  assert.equal(record.attempts[0].findings[0].status, 'implemented')
  assert.equal(record.attempts[0].commit, null)
  assert.equal(record.attempts[0].baseCommit, head)
  assert.ok(record.attempts[0].changedFiles.some(file => file.includes('file.txt')))
  assert.equal(git(f.config.source, 'rev-parse', 'HEAD'), head)
  assert.equal(fs.readFileSync(path.join(f.config.reports, record.sourceReport), 'utf8'), 'Review: improve tooltip behavior.\n')
  assert.equal(await processReport(f.config), 'handled')
})

test('rejected and deferred findings are logged as reviewed decisions, not implementation', async t => {
  for (const status of ['rejected', 'deferred']) {
    const f = fixture(t)
    fakeCodex(f, '', outcome(latestStatus(f.config.reports).id, status), false)
    assert.equal(await processReport(f.config), 'completed')
    assert.equal(readLedger(f.config.reports).reports[0].attempts[0].findings[0].status, status)
    assert.equal(git(f.config.source, 'status', '--porcelain'), '')
  }
})

test('command events split across stream chunks still validate verification', async t => {
  const f = fixture(t)
  const event = `${JSON.stringify({ type: 'item.completed', item: { type: 'command_execution', command: 'node --test fixture.test.mjs', exit_code: 0 } })}\n`
  fakeCodex(f, `process.stdout.write(${JSON.stringify(event.slice(0, 20))});setTimeout(()=>process.stdout.write(${JSON.stringify(event.slice(20))}),20);`, outcome(latestStatus(f.config.reports).id), false)
  assert.equal(await processReport(f.config), 'completed')
})

test('agent failure preserves partial changes and requires explicit retry', async t => {
  const f = fixture(t)
  fakeCodex(f, "fs.writeFileSync('file.txt','Partial change');process.exit(2);")
  await assert.rejects(processReport(f.config), /stopped/)
  assert.equal(latestStatus(f.config.reports).state, 'needs-review')
  assert.equal(fs.readFileSync(path.join(f.config.source, 'file.txt'), 'utf8'), 'Partial change')
  await assert.rejects(processReport(f.config), /unfinished/)
  assert.equal(readLedger(f.config.reports).reports[0].attempts.length, 1)
})

test('explicit retry preserves attempt history after reviewed clean failure', async t => {
  const f = fixture(t)
  fakeCodex(f, 'process.exit(2);')
  await assert.rejects(processReport(f.config), /stopped/)
  fakeCodex(f)
  assert.equal(await processReport(f.config, { retry: true }), 'completed')
  assert.deepEqual(readLedger(f.config.reports).reports[0].attempts.map(attempt => attempt.state), ['needs-review', 'completed'])
})

test('wrong report result never marks handled or discards source edits', async t => {
  const f = fixture(t)
  fakeCodex(f, "fs.writeFileSync('file.txt','Keep change');", outcome(reportId('Wrong report')))
  await assert.rejects(processReport(f.config), /identity/)
  assert.equal(latestStatus(f.config.reports).state, 'needs-review')
  assert.equal(fs.readFileSync(path.join(f.config.source, 'file.txt'), 'utf8'), 'Keep change')
})

test('changed latest report does not close either report incorrectly', async t => {
  const f = fixture(t)
  const oldId = latestStatus(f.config.reports).id
  fakeCodex(f, `fs.writeFileSync(${JSON.stringify(path.join(f.config.reports, 'latest.txt'))}, 'Different report');`)
  await assert.rejects(processReport(f.config), /Latest report changed/)
  assert.equal(latestStatus(f.config.reports).state, 'pending')
  const old = readLedger(f.config.reports).reports.find(report => report.id === oldId)
  assert.equal(old.attempts[0].state, 'needs-review')
})

test('externally edited ledger is preserved instead of overwritten', async t => {
  const f = fixture(t)
  const file = path.join(f.config.reports, 'handling-log.json')
  fakeCodex(f, `const l=JSON.parse(fs.readFileSync(${JSON.stringify(file)},'utf8'));l.reports[0].note='External update';fs.writeFileSync(${JSON.stringify(file)},JSON.stringify(l));`)
  await assert.rejects(processReport(f.config), /changed externally/)
  assert.equal(readLedger(f.config.reports).reports[0].note, 'External update')
  assert.equal(readLedger(f.config.reports).reports[0].attempts[0].state, 'processing')
})

test('timeout stops process group and retains partial edits', async t => {
  const f = fixture(t)
  f.config.maxMinutes = 0.003
  fakeCodex(f, "fs.writeFileSync('file.txt','Unfinished');setInterval(()=>{},1000);")
  await assert.rejects(processReport(f.config), /timed out/)
  assert.equal(latestStatus(f.config.reports).state, 'needs-review')
  assert.equal(fs.readFileSync(path.join(f.config.source, 'file.txt'), 'utf8'), 'Unfinished')
})

test('check mode is read-only, and concurrent maintenance lock blocks processing', async t => {
  const f = fixture(t)
  const helper = path.join(here, 'process-report.mjs')
  const target = path.join(f.config.source, 'scripts/maintenance')
  fs.mkdirSync(target, { recursive: true })
  for (const file of ['process-report.mjs', 'report-ledger.mjs', 'process-report.schema.json']) fs.copyFileSync(path.join(here, file), path.join(target, file))
  git(f.config.source, 'add', 'scripts')
  git(f.config.source, '-c', 'commit.gpgsign=false', 'commit', '-m', 'Install helper')
  const env = { ...process.env, BROWSEY_MAINTENANCE_CONFIG: f.configFile }
  const copied = path.join(target, path.basename(helper))
  const checked = spawnSync(process.execPath, [copied, '--check'], { encoding: 'utf8', env })
  assert.equal(checked.status, 0, checked.stderr)
  assert.equal(JSON.parse(checked.stdout).state, 'pending')
  assert.equal(fs.existsSync(f.config.state), false)
  fs.mkdirSync(f.config.state)
  const lock = spawn('flock', [path.join(f.config.state, 'run.lock'), process.execPath, '-e', 'console.log("locked");setTimeout(()=>{},700)'], { stdio: ['ignore', 'pipe', 'pipe'] })
  const finished = once(lock, 'close')
  await once(lock.stdout, 'data')
  const blocked = spawnSync(process.execPath, [copied], { encoding: 'utf8', env })
  assert.notEqual(blocked.status, 0)
  assert.match(blocked.stderr, /active/)
  await finished
  assert.equal(fs.existsSync(path.join(f.config.reports, 'handling-log.json')), false)
})

test('T3 action is explicitly manual and uses the repository helper', () => {
  const root = path.resolve(here, '../..')
  const config = JSON.parse(fs.readFileSync(path.join(root, 't3.json'), 'utf8'))
  const action = config.scripts.find(script => script.name === 'Process latest maintenance report')
  assert.equal(action.command, 'node scripts/maintenance/process-report.mjs')
  assert.equal(action.runOnWorktreeCreate, false)
})
