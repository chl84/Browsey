import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { sourceIdentity, fileSha256, verifyCandidate } from './candidate.mjs'

const exec = promisify(execFile)
test('candidate identity rejects stale source, another commit, altered binary and old manifests', async () => {
  const repo = await fs.mkdtemp(path.join(os.tmpdir(), 'browsey-candidate-'))
  try {
    await exec('git', ['init', '-q', repo])
    await fs.mkdir(path.join(repo, 'src'))
    const source = path.join(repo, 'src/main.rs')
    await fs.writeFile(source, 'fn main() {}')
    await exec('git', ['add', 'src/main.rs'], { cwd: repo })
    await exec('git', ['-c', 'user.name=Native test', '-c', 'user.email=native@example.invalid',
      '-c', 'commit.gpgsign=false', 'commit', '-qm', 'synthetic fixture'], { cwd: repo })
    const binary = path.join(repo, 'candidate')
    await fs.writeFile(binary, 'synthetic binary')
    const build = { schema: 2, feature: 'native-test', ...await sourceIdentity(repo), sha256: await fileSha256(binary) }
    const manifest = values => fs.writeFile(`${binary}.json`, JSON.stringify({ ...build, ...values }))
    await manifest({})
    await verifyCandidate(repo, binary)
    // Documentation/harness edits do not change what was compiled into the app.
    await fs.writeFile(path.join(repo, 'README.md'), 'documentation')
    await verifyCandidate(repo, binary)
    await fs.writeFile(source, 'fn main() { panic!(); }')
    await assert.rejects(verifyCandidate(repo, binary), /build inputs changed/)
    await fs.writeFile(source, 'fn main() {}')
    await fs.writeFile(path.join(repo, 'src/new.rs'), 'untracked build input')
    await assert.rejects(verifyCandidate(repo, binary), /build inputs changed/)
    await fs.unlink(path.join(repo, 'src/new.rs'))
    await fs.symlink('main.rs', path.join(repo, 'src/alias.rs'))
    assert.notEqual((await sourceIdentity(repo)).sourceSha256, build.sourceSha256)
    await fs.unlink(path.join(repo, 'src/alias.rs'))
    await fs.symlink('/synthetic/outside', path.join(repo, 'src/alias.rs'))
    await assert.rejects(sourceIdentity(repo))
    await fs.unlink(path.join(repo, 'src/alias.rs'))
    await manifest({ commit: 'another commit' })
    await assert.rejects(verifyCandidate(repo, binary), /another commit/)
    await manifest({ schema: 1 })
    await assert.rejects(verifyCandidate(repo, binary), /source identity/)
    await manifest({})
    await fs.writeFile(binary, 'altered binary')
    await assert.rejects(verifyCandidate(repo, binary), /changed after staging/)
  } finally { await fs.rm(repo, { recursive: true }) }
})
