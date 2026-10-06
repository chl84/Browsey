import assert from 'node:assert/strict'
import { kinds } from './scope.mjs'
const partPersistence = Symbol('partPersistence')

// Requirements describe this bounded case, not assumed filesystem support.
export function requirements(item) {
  const operation = item.id.split('-')[0]
  const common = { input: ['exact-text-input', 'file-create', 'rename', 'delete-cancel'],
    create: ['file-create', 'directory-create'], rename: ['file-rename', 'directory-rename'],
    delete: ['delete-cancel', 'file-delete', 'directory-delete'], undo: ['local-copy-undo-redo'],
    accessibility: ['owned-accessibility'], lifecycle: ['owned-window'],
    navigation: ['directory-read', 'owned-navigation', 'history-navigation', 'breadcrumb-navigation', 'refresh',
      `ui-${item.id.split('-')[1]}-navigation`, 'view-switch'] }
  return Object.fromEntries(item.providers.map((provider, index) => {
    if (operation === 'provider') return [provider, ['owned-provider-behavior', 'independent-source-preservation', 'explicit-refresh', 'bounded-fixtures']]
    if (operation === 'links') return [provider, ['owned-relative-broken-link-list-grid', 'explicit-symlink-clipboard-rejection', 'independent-hardlink-copy', 'same-inode-hardlink-move', 'owned-link-identity-digests', 'released-task-callbacks']]
    if (operation === 'trees') return [provider, item.id === 'trees-virtual-local' ? ['bounded-200-entry-list', 'virtualized-selection', 'all-selected-copy-digests', 'return-small-response', 'released-task-callbacks'] : ['reduced-device-fixture-first', 'empty-deep-wide-copy', 'bounded-independent-tree-digests', 'recursive-search-response', 'released-task-callbacks']]
    if (operation === 'contents') return [provider, ['zero-one-byte-binary-readback', 'sha256-byte-digests', 'real-copy-move', 'independent-source-destination-trees', 'released-task-callbacks', ...(item.providers.length === 1 || index === 0 ? ['successful-root-only-removal'] : [])]]
    if (operation === 'limits') return [provider, ['reserved-name-outcome', 'overlong-name-rejection', 'rename-component-rejection', 'independent-preservation', ...(provider === 'local' ? ['utf8-byte-name-limit', 'path-byte-limit', 'unsupported-filename-encoding-rejection'] : [])]]
    if (operation === 'names') return [provider, ['exact-name-list-grid', 'literal-uri-navigation', 'special-name-copy-move', 'independent-name-byte-preservation', 'emoji-source-rename', ...(provider === 'local' ? ['significant-whitespace-create-rename'] : [])]]
    if (operation === 'interruption') return [provider, ['owned-mid-operation-process-exit', 'independent-source-destination-trees', 'protected-original-recovery', 'safe-startup-empty-history', 'read-only-recovery-diagnostics']]
    if (operation === 'races') return [provider, ['owned-concurrent-writer', 'independent-source-destination-trees', 'truthful-uncertainty', 'released-task-callbacks', ...(item.id.endsWith('late-collision') ? ['protected-original-recovery'] : [])]]
    if (operation === 'iofaults') return [provider, [item.id.includes('-preparing-') ? 'preparation-cancellation' : 'bounded-owned-io-fault', 'independent-source-destination-trees', 'truthful-partial-counts', 'released-task-callbacks']]
    if (operation === 'access') return [provider, ['owned-read-write-denial', 'supported-read-only-copy', 'actionable-error-feedback', 'independent-source-destination-trees']]
    if (operation === 'moves') return [provider, ['real-move-cancellation-failure', 'independent-source-destination-trees', 'successful-root-only-removal', 'truthful-partial-counts', 'released-task-callbacks']]
    if (operation === 'overwrite') return [provider, ['overwrite-cancel-failure', 'independent-source-destination-trees', 'protected-original-recovery', 'truthful-completion']]
    if (operation === 'cancel') return [provider, ['real-copy-cancellation', 'independent-source-destination-trees', 'quiescent-writes', 'released-task-callbacks']]
    if (operation === 'progress') return [provider, ['visible-byte-progress', 'unknown-zero-totals', 'command-owned-completion', 'independent-source-destination-trees']]
    if (operation === 'batch') return [provider, ['batch-outcome-counts', 'batch-error-preservation', 'remaining-source-preservation', 'independent-source-destination-trees', 'refresh-reconciliation']]
    if (operation === 'guards') return [provider, ['unsafe-transfer-rejection', 'same-target-rejection', 'descendant-rejection', 'bounded-work', 'independent-preservation']]
    if (item.id.includes('-conflicts-')) return [provider, ['conflict-choice', 'nested-conflict-preservation', 'cross-kind-conflict-policy', 'independent-source-destination-trees']]
    if (item.id.startsWith('rename-edge-')) return [provider, ['file-rename', 'nonempty-directory-rename',
      'rename-collision', 'rename-extension', 'rename-case-only', 'rename-cancel', 'rename-repeat', 'rename-preservation']]
    if (operation === 'fileops') return [provider, ['mixed-copy', 'mixed-cut-paste', 'keyboard-context-operations',
      'no-selection', 'repeated-dispatch', 'permanent-delete-warning', 'cancelled-delete', 'independent-preservation']]
    if (operation === 'properties') return [provider, ['selected-properties', 'size-type', 'permission-capabilities',
      'ownership-capabilities', 'properties-tabs', 'properties-focus', 'independent-preservation']]
    if (operation === 'history') return [provider, ['local-move-undo-redo', 'local-rename-undo-redo',
      'local-delete-undo-redo', 'local-overwrite-undo-redo', 'redo-invalidation', 'history-50-limit', 'owned-restart-history']]
    if (operation === 'creation') return [provider, ['file-create', 'directory-create', 'empty-created-contents',
      'creation-invalid-name', 'creation-collision', 'creation-cancel', 'creation-escape', 'creation-focus', 'creation-preservation', 'view-switch']]
    if (operation === 'listing') return [provider, ['directory-read', 'ui-sort', 'ui-column-filters', 'ui-filter-reset',
      'ui-hidden-files', 'ui-name-filter', 'view-switch', 'scoped-recursive-search']]
    if (operation === 'selection') return [provider, ['ui-single-selection', 'ui-multiple-selection',
      'ui-range-selection', 'ui-arrow-selection', 'ui-select-all', 'selected-copy-readback',
      ...(item.id === 'selection-virtual-local' ? ['virtualized-selection'] : ['ui-empty-space', 'selection-navigation', 'view-switch'])]]
    if (operation !== 'copy' && operation !== 'move') return [provider, common[operation] ?? []]
    const source = item.providers.length === 1 || index === 0
    const destination = item.providers.length === 1 || index === 1
    return [provider, [...(source ? ['file-read', 'directory-read', ...(operation === 'move' ? ['source-removal'] : [])] : []),
      ...(destination ? ['file-write', 'directory-create'] : []), `ui-${operation}`,
      ...(item.id.startsWith(`${operation}-rich-`) ? ['empty-directory-transfer', 'nested-tree-transfer',
        'mixed-batch-transfer', 'independent-source-destination-trees'] : [])]]
  }))
}

export function createReport(plan, configured, manifest) {
  assert.equal(new Set(manifest.map(item => item.id)).size, manifest.length, 'Unique declared case IDs required')
  for (const item of manifest) {
    assert.ok(item.providers.length && item.providers.every(kind => plan.targets.some(target => target.kind === kind)),
      'Case providers must belong to the selected plan')
    assert.ok(Object.values(requirements(item)).every(caps => caps.length), 'Every case must declare requirements')
    if (item.partIds) assert.ok(Array.isArray(item.partIds) && item.partIds.length > 0 && item.partIds.length <= 128
      && item.partIds.every(id => typeof id === 'string' && id.length > 0)
      && new Set(item.partIds).size === item.partIds.length, 'Unique bounded declared parts required')
  }
  return { schema: 3, runId: plan.runId, started: new Date().toISOString(), status: 'RUNNING',
    retryPolicy: 'No automatic case, mutation or session-closure retry', setup: [],
    targets: Object.fromEntries(kinds.map(kind => [kind, plan.targets.some(target => target.kind === kind)
      ? 'NOT_RUN' : configured.includes(kind) ? 'DEFERRED' : 'NOT_CONFIGURED'])),
    cases: manifest.map(item => ({ ...item, requirements: requirements(item), status: 'NOT_RUN',
      parts: (item.partIds ?? (/^(copy|move)-/.test(item.id) ? ['file', 'directory'] : []))
        .map(id => ({ id, status: 'NOT_RUN', ui: 'NOT_SENT' })) })) }
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
  result[partPersistence] = persist
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
  } finally { delete result[partPersistence]; await persist() }
}

export async function recordPart(result, id, action) {
  let part = result.parts?.find(item => item.id === id)
  if (part) assert.equal(part.status, 'NOT_RUN', 'A partial operation must not be retried')
  else { part = { id, status: 'NOT_RUN', ui: 'NOT_SENT' }; (result.parts ??= []).push(part) }
  part.status = 'RUNNING'
  await result[partPersistence]?.()
  try { await action(part); part.status = 'PASS'; part.phase = result.phase }
  catch (error) { failed(part, error, result.phase); throw error }
  finally { await result[partPersistence]?.() }
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
