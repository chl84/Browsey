import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('./performance-workloads.sh', import.meta.url))
test('dry run lists only the five explicitly scoped tests and creates no data', () => {
  const root = mkdtempSync(join(tmpdir(), 'browsey-performance-script-'))
  try {
    const output = execFileSync('bash', [script, '--directory', root, '--dry-run'], { encoding: 'utf8' })
    assert.equal(output.match(/Would run exactly:/g)?.length, 5)
    assert.match(output, /profile: release/)
    assert.match(output, /recursive_search_workloads/)
    assert.doesNotMatch(output, /real_onedrive|native_drag|cargo test.*--ignored\s*$/m)
    assert.deepEqual(readdirSync(root), [])
  } finally { rmSync(root, { recursive: true }) }
})
test('debug runs are explicitly labelled, not presented as optimized results', () => {
  const output = execFileSync('bash', [script, '--debug', '--dry-run'], { encoding: 'utf8' })
  assert.match(output, /profile: debug/)
})
test('invalid options and nonexistent parents fail before running tests', () => {
  for (const args of [['--unexpected'], ['--directory'], ['--directory', '/browsey-nonexistent-performance-parent', '--dry-run']]) {
    const result = spawnSync('bash', [script, ...args], { encoding: 'utf8' })
    assert.equal(result.status, 2)
    assert.doesNotMatch(result.stdout, /running|Would run/)
  }
})
