import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { noLinks, inside } from './scope.mjs'
import { regularFile } from './fixtures.mjs'

const exec = promisify(execFile)
const buildInput = raw => /^(src\/|vendor\/|resources\/|capabilities\/|\.cargo\/|frontend\/src\/|frontend\/public\/)/.test(raw)
  || /^(Cargo\.(toml|lock)|build\.rs|tauri\.conf\.json|THIRD_PARTY_NOTICES|frontend\/(package(-lock)?\.json|index\.html|vite\.config\.[^/]+|tsconfig[^/]*\.json))$/.test(raw)

export async function sourceIdentity(repo) {
  const commit = (await exec('git', ['rev-parse', 'HEAD'], { cwd: repo })).stdout.trim()
  const dirty = Boolean((await exec('git', ['status', '--porcelain'], { cwd: repo })).stdout.trim())
  const files = [...new Set((await exec('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
    { cwd: repo, maxBuffer: 8 * 1024 * 1024 })).stdout.split('\0').filter(buildInput))].sort()
  const hash = createHash('sha256')
  for (const raw of files) {
    const absolute = path.join(repo, raw)
    // The checkout includes a PDFium ABI symlink. Hash its spelling and bounded
    // repository referent; fixture symlinks remain forbidden by the run guard.
    await noLinks(path.dirname(absolute), fs)
    hash.update(raw).update('\0')
    if ((await fs.lstat(absolute)).isSymbolicLink()) {
      const resolved = await fs.realpath(absolute)
      assert.ok(inside(path.resolve(repo), resolved), 'Build input symlink leaves the checkout')
      hash.update('link\0').update(await fs.readlink(absolute)).update('\0')
      await noLinks(resolved, fs)
      hash.update(await fs.readFile(resolved))
    } else hash.update('file\0').update(await fs.readFile(absolute))
    hash.update('\0')
  }
  return { commit, dirty, sourceSha256: hash.digest('hex') }
}

export async function fileSha256(raw) {
  await noLinks(raw, fs)
  const handle = await fs.open(raw, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW)
  try {
    assert.ok((await handle.stat()).isFile(), 'Expected a regular candidate file')
    const hash = createHash('sha256')
    for await (const chunk of handle.createReadStream({ autoClose: false })) hash.update(chunk)
    return hash.digest('hex')
  } finally { await handle.close() }
}

export async function verifyCandidate(repo, binary) {
  const build = JSON.parse((await regularFile(`${binary}.json`)).text)
  assert.equal(build.schema, 2, 'Rebuild the candidate with source identity')
  assert.equal(build.feature, 'native-test')
  assert.equal(build.sha256, await fileSha256(binary), 'Candidate changed after staging')
  const current = await sourceIdentity(repo)
  assert.equal(build.commit, current.commit, 'Candidate belongs to another commit; rebuild')
  assert.equal(build.sourceSha256, current.sourceSha256, 'Candidate build inputs changed; rebuild')
  return build
}
