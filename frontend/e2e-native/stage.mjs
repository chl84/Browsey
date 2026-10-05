import * as fs from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

// Development build artifacts only. Never copy into ~/.local or use the installed
// application/resources. One shared Cargo cache, not a new worktree per test.
const repo = fileURLToPath(new URL('../..', import.meta.url))
const destination = path.join(repo, 'target/native-test')
const exec = promisify(execFile)
await fs.mkdir(destination, { recursive: true })
await fs.copyFile(path.join(repo, 'target/debug/browsey'), path.join(destination, 'browsey'))
await fs.chmod(path.join(destination, 'browsey'), 0o755)
// Stage resources from this checkout; do not fall back to personal installation.
await fs.cp(path.join(repo, 'resources'), path.join(destination, 'resources'), { recursive: true })
await fs.copyFile(path.join(repo, 'THIRD_PARTY_NOTICES'), path.join(destination, 'THIRD_PARTY_NOTICES'))
const hash = createHash('sha256')
const binary = await fs.open(path.join(destination, 'browsey'), 'r')
try { for await (const chunk of binary.createReadStream({ autoClose: false })) hash.update(chunk) }
finally { await binary.close() }
const commit = (await exec('git', ['rev-parse', 'HEAD'], { cwd: repo })).stdout.trim()
const dirty = Boolean((await exec('git', ['status', '--porcelain'], { cwd: repo })).stdout.trim())
await fs.writeFile(path.join(destination, 'browsey.json'), JSON.stringify({ schema: 1, feature: 'native-test',
  commit, dirty, builtAt: new Date().toISOString(), sha256: hash.digest('hex'), profile: 'debug' }, null, 2))
console.log('Staged scoped candidate in target/native-test. Nothing installed or launched.')
