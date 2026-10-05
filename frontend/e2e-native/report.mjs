import assert from 'node:assert/strict'
import { kinds } from './scope.mjs'

// Requirements describe this bounded case, not assumed filesystem support.
export function requirements(item) {
  const operation = item.id.split('-')[0]
  const common = { input: ['exact-text-input', 'file-create', 'rename', 'delete-cancel'],
    create: ['file-create', 'directory-create'], rename: ['file-rename', 'directory-rename'],
    delete: ['delete-cancel', 'file-delete', 'directory-delete'], undo: ['local-copy-undo-redo'],
    accessibility: ['owned-accessibility'], lifecycle: ['owned-window'] }
  return Object.fromEntries(item.providers.map((provider, index) => {
    if (operation !== 'copy' && operation !== 'move') return [provider, common[operation] ?? []]
    const source = item.providers.length === 1 || index === 0
    const destination = item.providers.length === 1 || index === 1
    return [provider, [...(source ? ['file-read', 'directory-read', ...(operation === 'move' ? ['source-removal'] : [])] : []),
      ...(destination ? ['file-write', 'directory-create'] : []), `ui-${operation}`]]
  }))
}

export function createReport(plan, configured, manifest) {
  assert.equal(new Set(manifest.map(item => item.id)).size, manifest.length, 'Unique declared case IDs required')
  for (const item of manifest) {
    assert.ok(item.providers.length && item.providers.every(kind => plan.targets.some(target => target.kind === kind)),
      'Case providers must belong to the selected plan')
    assert.ok(Object.values(requirements(item)).every(caps => caps.length), 'Every case must declare requirements')
  }
  return { schema: 3, runId: plan.runId, started: new Date().toISOString(), status: 'RUNNING',
    retryPolicy: 'No automatic case, mutation or session-closure retry', setup: [],
    targets: Object.fromEntries(kinds.map(kind => [kind, plan.targets.some(target => target.kind === kind)
      ? 'NOT_RUN' : configured.includes(kind) ? 'DEFERRED' : 'NOT_CONFIGURED'])),
    cases: manifest.map(item => ({ ...item, requirements: requirements(item), status: 'NOT_RUN',
      parts: /^(copy|move)-/.test(item.id) ? ['file', 'directory'].map(id => ({ id, status: 'NOT_RUN', ui: 'NOT_SENT' })) : [] })) }
}

export function failure(error, phase) {
  const kind = error.failureKind
  if (kind === 'APP_REPORTED_ERROR') return { status: 'FAIL', failureKind: kind, failureOrigin: 'app-reported' }
  if (kind === 'CANDIDATE_EXITED') return { status: 'FAIL', failureKind: kind, failureOrigin: 'candidate-process' }
  if (kind === 'FIXTURE_IO' || phase === 'setup' || ['EACCES', 'EPERM', 'EIO', 'ENOSPC'].includes(error.code)) {
    return { status: 'BLOCKED', failureKind: kind ?? (phase === 'setup' ? 'FIXTURE_SETUP' : 'FIXTURE_IO'), failureOrigin: 'harness' }
  }
  if (phase === 'verification' && error.code === 'ERR_ASSERTION') {
    return { status: 'FAIL', failureKind: 'RESULT_MISMATCH', failureOrigin: 'candidate-result' }
  }
  if (kind) return { status: 'BLOCKED', failureKind: kind, failureOrigin: 'harness' }
  return { status: 'BLOCKED', failureKind: 'UNCLASSIFIED_UI_OR_RESULT', failureOrigin: 'undetermined' }
}

function failed(result, error, phase) {
  Object.assign(result, failure(error, phase), { phase, error: error.message })
}

export async function recordSetup(report, metadata, action, persist = async () => {}) {
  assert.ok(!report.setup.some(item => item.id === metadata.id), 'Setup must not be retried')
  const result = { ...metadata, status: 'RUNNING', phase: 'setup' }
  report.setup.push(result)
  await persist()
  try { const value = await action(); result.status = 'PASS'; return value }
  catch (error) { failed(result, error, 'setup'); throw error }
  finally { await persist() }
}

export async function recordCase(report, metadata, action, { before = async () => {}, after = async () => {},
  resolveFailure = async error => error, persist = async () => {} } = {}) {
  const result = report.cases.find(item => item.id === metadata.id)
  assert.ok(result && result.status === 'NOT_RUN', 'Case must be declared and must not be retried')
  assert.deepEqual(metadata.providers, result.providers, 'Runtime providers must match declared direction')
  result.status = 'RUNNING'
  result.phase = 'ownership'
  await persist()
  try {
    await before()
    result.phase = 'ui'
    await action(result)
    result.phase = 'ownership'
    await after()
    result.status = 'PASS'
  } catch (error) {
    const observed = await resolveFailure(error)
    failed(result, observed, result.phase)
    throw observed
  } finally { await persist() }
}

export async function recordPart(result, id, action) {
  let part = result.parts?.find(item => item.id === id)
  if (part) assert.equal(part.status, 'NOT_RUN', 'A partial operation must not be retried')
  else { part = { id, status: 'NOT_RUN', ui: 'NOT_SENT' }; (result.parts ??= []).push(part) }
  part.status = 'RUNNING'
  try { await action(part); part.status = 'PASS'; part.phase = result.phase }
  catch (error) { failed(part, error, result.phase); throw error }
}

export function finishReport(report, error) {
  if (error) {
    const observed = report.cases.find(item => ['FAIL', 'BLOCKED'].includes(item.status))
      ?? report.setup.find(item => item.status === 'BLOCKED') ?? failure(error, 'setup')
    report.status = report.cases.some(item => item.status === 'FAIL') ? 'FAIL' : 'BLOCKED'
    report.failureKind = observed.failureKind
    report.failureOrigin = observed.failureOrigin
    report.error = error.message
  } else {
    report.status = report.cases.length && report.cases.every(item => item.status === 'PASS')
      && report.setup.every(item => item.status === 'PASS') ? 'PASS' : 'BLOCKED'
  }
}

export function summarizeProviders(report) {
  report.providers = Object.fromEntries(kinds.map(kind => {
    const declared = report.cases.filter(item => item.providers.includes(kind))
    const setup = report.setup.filter(item => item.providers.includes(kind))
    const selected = !['DEFERRED', 'NOT_CONFIGURED'].includes(report.targets[kind])
    const counts = Object.fromEntries(['PASS', 'FAIL', 'BLOCKED', 'NOT_RUN', 'RUNNING'].map(status =>
      [status, declared.filter(item => item.status === status).length]))
    const status = !selected ? report.targets[kind] : counts.FAIL ? 'FAIL'
      : !declared.length || counts.BLOCKED || counts.NOT_RUN || counts.RUNNING || setup.some(item => item.status !== 'PASS')
        || report.status === 'BLOCKED' ? 'BLOCKED' : 'PASS'
    report.targets[kind] = status
    const names = [...new Set(declared.flatMap(item => item.requirements[kind]))]
    return [kind, { status, counts, setup: setup.map(({ id, status, failureKind }) => ({ id, status, failureKind })),
      cases: declared.map(({ id, status, phase, failureKind, failureOrigin, parts }) => ({ id, status, phase,
        failureKind, failureOrigin, parts: parts.map(({ id, status, phase, ui }) => ({ id, status, phase, ui })) })),
      capabilities: Object.fromEntries(names.map(name => {
        const cases = declared.filter(item => item.requirements[kind].includes(name))
        return [name, { status: cases.some(item => item.status === 'FAIL') ? 'FAIL'
          : cases.every(item => item.status === 'PASS') ? 'PASS'
            : cases.some(item => item.status === 'BLOCKED') ? 'BLOCKED' : 'NOT_RUN',
        requiredBy: cases.map(item => item.id), passedBy: cases.filter(item => item.status === 'PASS').map(item => item.id) }]
      })) }]
  }))
}
