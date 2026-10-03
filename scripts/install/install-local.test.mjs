import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const sourceDir = dirname(fileURLToPath(import.meta.url))

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'browsey installer tests-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const repo = join(root, 'checkout')
  const local = join(root, 'local')
  const commands = join(root, 'commands')
  for (const folder of ['scripts/install', 'frontend/node_modules/.bin', 'resources/pdfium-linux-x64/lib']) {
    mkdirSync(join(repo, folder), { recursive: true })
  }
  mkdirSync(commands)
  for (const name of ['install-local.sh', 'browsey-launcher.sh']) {
    copyFileSync(join(sourceDir, name), join(repo, 'scripts/install', name))
  }
  writeFileSync(join(repo, 'resources/pdfium-linux-x64/lib/libpdfium.so'), 'pdfium fixture')
  writeFileSync(join(repo, 'resources/ignored.orig'), 'do not ship patch leftovers')
  writeFileSync(join(repo, 'THIRD_PARTY_NOTICES'), 'notices fixture')
  function executable(path, content) {
    writeFileSync(path, content)
    chmodSync(path, 0o755)
  }
  executable(join(commands, 'cargo'), '#!/bin/sh\nexit 0\n')
  executable(join(commands, 'ldd'), '#!/bin/sh\nif [ "${BROWSEY_TEST_MISSING_LIB:-}" = 1 ]; then echo "library => not found"; else echo "library => /lib/library"; fi\n')
  executable(join(repo, 'frontend/node_modules/.bin/tauri'), `#!/bin/sh
set -eu
test "$*" = 'build --no-bundle -- --locked'
if [ "\${BROWSEY_TEST_BUILD_FAILURE:-}" = 1 ]; then exit 42; fi
mkdir -p target/release
printf '#!/bin/sh\\nprintf new-browsey\\n' > target/release/browsey
chmod 755 target/release/browsey
`)
  function run(args = [], extraEnv = {}) {
    return spawnSync('bash', [join(repo, 'scripts/install/install-local.sh'), ...args], {
      cwd: tmpdir(), // The caller need not be at the repository root.
      env: { ...process.env, PATH: `${commands}:${process.env.PATH}`, BROWSEY_LOCAL_DIR: local, ...extraEnv },
      encoding: 'utf8',
    })
  }
  function installOld() {
    const old = join(local, 'opt/browsey/usr')
    mkdirSync(join(old, 'bin'), { recursive: true })
    mkdirSync(join(old, 'share/applications'), { recursive: true })
    writeFileSync(join(old, 'bin/browsey'), 'old binary')
    writeFileSync(join(old, 'share/applications/Browsey.desktop'), 'existing desktop integration')
  }
  return { root, repo, local, commands, executable, run, installOld }
}

test('dry run and help do not build or create an installation', t => {
  const f = fixture(t)
  for (const option of ['--dry-run', '--help']) {
    const result = f.run([option])
    assert.equal(result.status, 0, result.stderr)
  }
  assert.equal(existsSync(f.local), false)
  assert.equal(existsSync(join(f.repo, 'target')), false)
})

test('fresh installation includes frontend build, resources and a relocatable launcher', t => {
  const f = fixture(t)
  const result = f.run()
  assert.equal(result.status, 0, result.stderr)
  const prefix = join(f.local, 'opt/browsey/usr')
  assert.equal(readFileSync(join(prefix, 'bin/browsey'), 'utf8'), readFileSync(join(f.repo, 'target/release/browsey'), 'utf8'))
  assert.equal(readFileSync(join(prefix, 'lib/Browsey/THIRD_PARTY_NOTICES'), 'utf8'), 'notices fixture')
  assert.equal(readFileSync(join(prefix, 'bin/libpdfium.so'), 'utf8'), 'pdfium fixture')
  assert.equal(existsSync(join(prefix, 'lib/Browsey/resources/ignored.orig')), false)
  const launch = spawnSync(join(f.local, 'bin/browsey'), [], { encoding: 'utf8' })
  assert.equal(launch.status, 0, launch.stderr)
  assert.equal(launch.stdout, 'new-browsey')
})

test('upgrade retains the old installation and preserves desktop integration', t => {
  const f = fixture(t)
  f.installOld()
  const result = f.run()
  assert.equal(result.status, 0, result.stderr)
  const backup = readdirSync(join(f.local, 'opt')).find(name => name.startsWith('.browsey-backup.'))
  assert.ok(backup)
  assert.equal(readFileSync(join(f.local, 'opt', backup, 'app/usr/bin/browsey'), 'utf8'), 'old binary')
  assert.equal(readFileSync(join(f.local, 'opt/browsey/usr/share/applications/Browsey.desktop'), 'utf8'), 'existing desktop integration')
  assert.equal(readdirSync(join(f.local, 'opt')).some(name => /^\.browsey-install\.[A-Za-z0-9]{6}$/.test(name)), false)
})

test('failed build cannot install a stale release binary', t => {
  const f = fixture(t)
  f.installOld()
  mkdirSync(join(f.repo, 'target/release'), { recursive: true })
  writeFileSync(join(f.repo, 'target/release/browsey'), 'stale binary')
  const result = f.run([], { BROWSEY_TEST_BUILD_FAILURE: '1' })
  assert.notEqual(result.status, 0)
  assert.equal(readFileSync(join(f.local, 'opt/browsey/usr/bin/browsey'), 'utf8'), 'old binary')
})

test('missing runtime libraries leave the existing installation untouched', t => {
  const f = fixture(t)
  f.installOld()
  const result = f.run([], { BROWSEY_TEST_MISSING_LIB: '1' })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Missing runtime libraries/)
  assert.equal(readFileSync(join(f.local, 'opt/browsey/usr/bin/browsey'), 'utf8'), 'old binary')
})

test('invalid options, broad paths and symlink installations are refused', t => {
  const f = fixture(t)
  for (const args of [['--skip-build'], ['--dry-run', '--help']]) {
    assert.notEqual(f.run(args).status, 0)
  }
  for (const path of ['/', process.env.HOME, f.repo, 'relative', `${process.env.HOME}/.`]) {
    assert.notEqual(f.run(['--dry-run'], { BROWSEY_LOCAL_DIR: path }).status, 0)
  }
  mkdirSync(join(f.local, 'opt'), { recursive: true })
  symlinkSync(f.repo, join(f.local, 'opt/browsey'))
  const result = f.run()
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Refusing to replace a symlink/)
  assert.equal(existsSync(join(f.repo, 'target')), false)
})

test('a failed directory switch restores the old installation', t => {
  const f = fixture(t)
  f.installOld()
  f.executable(join(f.commands, 'mv'), `#!/bin/bash
set -eu
if [[ "$1" == -- ]]; then shift; fi
if [[ "$1" == *'/.browsey-install.'* && "$2" == */opt/browsey ]]; then exit 1; fi
exec /usr/bin/mv "$@"
`)
  const result = f.run()
  assert.notEqual(result.status, 0)
  assert.equal(readFileSync(join(f.local, 'opt/browsey/usr/bin/browsey'), 'utf8'), 'old binary')
  assert.equal(readdirSync(join(f.local, 'opt')).some(name => /^\.browsey-install\.[A-Za-z0-9]{6}$/.test(name)), false)
})

test('a staging failure leaves the old installation intact and removes temporary files', t => {
  const f = fixture(t)
  f.installOld()
  rmSync(join(f.repo, 'resources/pdfium-linux-x64/lib/libpdfium.so'))
  const result = f.run()
  assert.notEqual(result.status, 0)
  assert.equal(readFileSync(join(f.local, 'opt/browsey/usr/bin/browsey'), 'utf8'), 'old binary')
  assert.equal(readdirSync(join(f.local, 'opt')).some(name => /^\.browsey-install\.[A-Za-z0-9]{6}$/.test(name)), false)
})

test('an active installation lock blocks a second build', t => {
  const f = fixture(t)
  mkdirSync(join(f.local, 'opt'), { recursive: true })
  const lock = join(f.local, 'opt/.browsey-install.lock')
  const result = spawnSync('flock', [lock, 'bash', join(f.repo, 'scripts/install/install-local.sh')], {
    env: { ...process.env, BROWSEY_LOCAL_DIR: f.local }, encoding: 'utf8',
  })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Another Browsey installation/)
  assert.equal(existsSync(join(f.repo, 'target')), false)
})

test('an existing rustup cargo takes precedence over an unconfigured PATH shim', t => {
  const f = fixture(t)
  const rustHome = join(f.root, 'rust user')
  mkdirSync(join(rustHome, '.cargo/bin'), { recursive: true })
  f.executable(join(rustHome, '.cargo/bin/cargo'), '#!/bin/sh\nprintf rustup-cargo\n')
  f.executable(join(f.commands, 'cargo'), '#!/bin/sh\nexit 42\n')
  const tauriPath = join(f.repo, 'frontend/node_modules/.bin/tauri')
  const tauri = readFileSync(tauriPath, 'utf8')
  writeFileSync(tauriPath, tauri.replace('set -eu\n', 'set -eu\ntest "$(cargo)" = rustup-cargo\n'))
  const result = f.run([], { HOME: rustHome })
  assert.equal(result.status, 0, result.stderr)
})
