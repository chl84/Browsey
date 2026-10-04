import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'

export const reportId = text => `sha256:${createHash('sha256').update(text).digest('hex')}`

export function rejectSymlinks(directory) {
  let current = path.parse(directory).root
  for (const component of directory.slice(current.length).split(path.sep).filter(Boolean)) {
    current = path.join(current, component)
    try {
      if (fs.lstatSync(current).isSymbolicLink()) throw new Error(`Symlink path refused: ${current}`)
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
    }
  }
}

export function readText(file, limit = 128 * 1024, missing = false) {
  let stat
  try { stat = fs.lstatSync(file) } catch (error) {
    if (missing && error.code === 'ENOENT') return null
    throw error
  }
  if (!stat.isFile() || stat.size > limit) throw new Error(`Expected a bounded regular file: ${file}`)
  return fs.readFileSync(file, 'utf8')
}

export function readLedger(directory) {
  rejectSymlinks(directory)
  const text = readText(path.join(directory, 'handling-log.json'), 4 * 1024 * 1024, true)
  const ledger = text === null ? { version: 1, reports: [] } : JSON.parse(text)
  if (ledger.version !== 1 || !Array.isArray(ledger.reports)) throw new Error('Invalid handling log')
  const ids = new Set()
  for (const report of ledger.reports) {
    if (!/^sha256:[a-f0-9]{64}$/.test(report.id) || ids.has(report.id) || !Array.isArray(report.attempts) || !report.attempts.length) {
      throw new Error('Invalid or duplicate report in handling log')
    }
    ids.add(report.id)
    for (const attempt of report.attempts) {
      if (!['processing', 'completed', 'needs-review'].includes(attempt.state) || !Array.isArray(attempt.findings)) {
        throw new Error('Invalid handling attempt')
      }
    }
  }
  return ledger
}

export function latestStatus(directory) {
  rejectSymlinks(directory)
  const text = readText(path.join(directory, 'latest.txt'))
  if (!text.trim()) throw new Error('Latest report is empty')
  const id = reportId(text)
  const ledger = readLedger(directory)
  const record = ledger.reports.find(report => report.id === id)
  const state = record?.attempts.some(attempt => attempt.state === 'completed')
    ? 'handled' : record ? 'needs-review' : 'pending'
  return { id, text, ledger, record, state }
}

export function atomicWrite(file, text) {
  rejectSymlinks(path.dirname(file))
  readText(file, 4 * 1024 * 1024, true)
  const temporary = `${file}.tmp-${process.pid}`
  const descriptor = fs.openSync(temporary, 'wx', 0o600)
  try {
    try {
      fs.writeFileSync(descriptor, text)
      fs.fsyncSync(descriptor)
    } finally { fs.closeSync(descriptor) }
    fs.renameSync(temporary, file)
  } finally {
    if (fs.existsSync(temporary)) fs.unlinkSync(temporary)
  }
}

export function writeLedger(directory, ledger) {
  atomicWrite(path.join(directory, 'handling-log.json'), `${JSON.stringify(ledger, null, 2)}\n`)
}

export function archiveReport(directory, text) {
  const file = `report-${reportId(text).slice(7)}.txt`
  const target = path.join(directory, file)
  const existing = readText(target, 128 * 1024, true)
  if (existing !== null && existing !== text) throw new Error('Report archive identity mismatch')
  if (existing === null) fs.writeFileSync(target, text, { flag: 'wx', mode: 0o600 })
  return file
}

export function validateOutcome(value, id, commands) {
  if (!value || value.reportId !== id || typeof value.summary !== 'string' || !value.summary.trim() || !Array.isArray(value.findings) || value.findings.length > 3) {
    throw new Error('Invalid processing result or wrong report identity')
  }
  for (const finding of value.findings) {
    if (!finding || typeof finding.title !== 'string' || !finding.title.trim() ||
        !['implemented', 'rejected', 'deferred'].includes(finding.status) ||
        typeof finding.reason !== 'string' || !finding.reason.trim() ||
        !Array.isArray(finding.files) || finding.files.some(file => typeof file !== 'string') ||
        !Array.isArray(finding.remainingRisks) || finding.remainingRisks.some(risk => typeof risk !== 'string') ||
        !Array.isArray(finding.verification)) throw new Error('Invalid finding disposition')
    for (const check of finding.verification) {
      if (!check || typeof check.command !== 'string' || !check.command.trim() ||
          !['passed', 'failed', 'expected-failure', 'not-run'].includes(check.result) || typeof check.details !== 'string') {
        throw new Error('Invalid verification result')
      }
      const matching = commands.filter(command => command.command.includes(check.command))
      if (check.result === 'passed' && matching.at(-1)?.exitCode !== 0) {
        throw new Error(`No successful command evidence for: ${check.command}`)
      }
      if (check.result === 'expected-failure' && (!matching.some(command => typeof command.exitCode === 'number' && command.exitCode !== 0) ||
          !finding.verification.some(other => other.command === check.command && other.result === 'passed'))) {
        throw new Error('Expected pre-fix failure needs failed command evidence and a successful rerun')
      }
    }
    if (finding.status === 'implemented' && (!finding.verification.some(check => check.result === 'passed') || finding.verification.some(check => check.result === 'failed'))) {
      throw new Error('Implemented findings require successful verification and no failed checks')
    }
  }
  return value
}
