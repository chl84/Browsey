import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const runner = join(repo, 'scripts/maintenance/check-semgrep.mjs')
const violations = `
fn bad(input: Result<(), ExampleError>, error: ExampleError) {
    let _ = ExampleError::from_external_message(error.to_string());
    let _ = input.map_err(|error| ExampleError::new(ErrorCode::Failed, error.to_string()));
}
impl From<ExampleError> for String {
    fn from(error: ExampleError) -> Self { format!("{error}") }
}
`

function fixture(t, files) {
  const root = mkdtempSync(join(tmpdir(), 'browsey-semgrep-regression-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, '.semgrep'))
  for (const name of ['typed-errors.yml', 'typed-errors-blocking.yml']) {
    copyFileSync(join(repo, '.semgrep', name), join(root, '.semgrep', name))
  }
  for (const [name, contents] of Object.entries(files)) {
    mkdirSync(dirname(join(root, name)), { recursive: true })
    writeFileSync(join(root, name), contents)
  }
  // Anchored rule paths are project-relative. Match the real Git-checkout
  // environment so a folder scan cannot pass simply by selecting no files.
  for (const args of [['init', '--quiet'], ['add', '--', 'src']]) {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' })
    assert.ifError(result.error)
    assert.equal(result.status, 0, result.stderr)
  }
  return root
}

function scan(root, mode) {
  const result = spawnSync(process.execPath, [runner, mode, '--json'], {
    cwd: root,
    encoding: 'utf8',
    timeout: 60000,
  })
  assert.ifError(result.error)
  assert.equal(result.signal, null, result.stderr)
  return result
}

for (const mode of ['advisory', 'blocking']) {
  test(`${mode} detects all three error seams and returns the intended exit code`, t => {
    const root = fixture(t, { 'src/commands/positive.rs': violations })
    const result = scan(root, mode)
    const data = JSON.parse(result.stdout)
    assert.equal(result.status, mode === 'blocking' ? 1 : 0, result.stderr)
    assert.deepEqual(data.errors, [])
    assert.equal(data.results.length, 3)
    assert.equal(new Set(data.results.map(item => item.check_id)).size, 3)
    assert.deepEqual(data.paths.scanned, ['src/commands/positive.rs'])
  })
}

test('clean typed conversions pass while test-only fixture violations remain excluded', t => {
  const root = fixture(t, {
    'src/commands/negative.rs': 'fn good(input: Result<(), ExampleError>) { let _ = input.map_err(TypedError::from); }',
    'src/commands/tests.rs': violations,
    'src/commands/tests/fixture.rs': violations,
    'src/commands/fixture_test.rs': violations,
  })
  const result = scan(root, 'blocking')
  const data = JSON.parse(result.stdout)
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(data.errors, [])
  assert.deepEqual(data.results, [])
  assert.ok(data.paths.scanned.includes('src/commands/negative.rs'))
})

test('blocking scan fails on invalid configuration, not only matching code', t => {
  const root = fixture(t, { 'src/commands/negative.rs': 'fn good() {}' })
  writeFileSync(join(root, '.semgrep/typed-errors-blocking.yml'), 'rules: [{id: invalid}]\n')
  const result = scan(root, 'blocking')
  assert.notEqual(result.status, 0, result.stderr)
})
