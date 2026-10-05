import * as fs from 'node:fs/promises'
import assert from 'node:assert/strict'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { sourceIdentity, fileSha256 } from './candidate.mjs'

// Development build artifacts only. Never copy into ~/.local or use the installed
// application/resources. One shared Cargo cache, not a new worktree per test.
const repo = fileURLToPath(new URL('../..', import.meta.url))
const destination = path.join(repo, 'target/native-test')
await fs.mkdir(destination, { recursive: true })
const identityPath = path.join(destination, 'build-inputs.json')
if (process.argv[2] === '--prepare') {
  await fs.writeFile(identityPath, JSON.stringify(await sourceIdentity(repo)))
} else {
  const before = JSON.parse(await fs.readFile(identityPath, 'utf8'))
  const after = await sourceIdentity(repo)
  assert.equal(before.commit, after.commit, 'Commit changed while building; rebuild')
  assert.equal(before.sourceSha256, after.sourceSha256, 'Build inputs changed while building; rebuild')
  await fs.copyFile(path.join(repo, 'target/debug/browsey'), path.join(destination, 'browsey'))
  await fs.chmod(path.join(destination, 'browsey'), 0o755)
  // Stage resources from this checkout; do not fall back to personal installation.
  await fs.cp(path.join(repo, 'resources'), path.join(destination, 'resources'), { recursive: true })
  await fs.copyFile(path.join(repo, 'THIRD_PARTY_NOTICES'), path.join(destination, 'THIRD_PARTY_NOTICES'))
  await fs.writeFile(path.join(destination, 'browsey.json'), JSON.stringify({ schema: 2, feature: 'native-test',
    ...before, builtAt: new Date().toISOString(), sha256: await fileSha256(path.join(destination, 'browsey')), profile: 'debug' }, null, 2))
  console.log('Staged scoped candidate in target/native-test. Nothing installed or launched.')

}
