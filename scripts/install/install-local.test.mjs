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
  const notificationLog = join(root, 'notifications')
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
  // Never send real desktop notifications from installer fixtures.
  for (const name of ['omarchy', 'notify-send']) {
    executable(join(commands, name), `#!/bin/sh
printf '%s\\0' '${name}' "$@" >> "$BROWSEY_TEST_NOTIFICATION_LOG"
if [ '${name}' = omarchy ]; then exit "\${BROWSEY_TEST_OMARCHY_STATUS:-0}"; fi
exit "\${BROWSEY_TEST_NOTIFY_STATUS:-0}"
`)
  }
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
    const result = spawnSync('bash', [join(repo, 'scripts/install/install-local.sh'), ...args], {
      cwd: tmpdir(), // The caller need not be at the repository root.
      env: { ...process.env, PATH: `${commands}:${process.env.PATH}`, BROWSEY_LOCAL_DIR: local,
        BROWSEY_TEST_NOTIFICATION_LOG: notificationLog, ...extraEnv },
      encoding: 'utf8',
    })
    if (result.status !== 0 || args.includes('--dry-run') || args.includes('--help')) {
      assert.equal(existsSync(notificationLog), false, 'No success notification before successful installation')
    }
    return result
  }
  function installOld() {
    const old = join(local, 'opt/browsey/usr')
    mkdirSync(join(old, 'bin'), { recursive: true })
    mkdirSync(join(old, 'share/applications'), { recursive: true })
    writeFileSync(join(old, 'bin/browsey'), 'old binary')
    writeFileSync(join(old, 'share/applications/Browsey.desktop'), 'existing desktop integration')
  }
  return { root, repo, local, commands, notificationLog, executable, run, installOld }
}

test('dry run and help do not build or create an installation', t => {
  const f = fixture(t)
  for (const option of ['--dry-run', '--help']) {
    const result = f.run([option])
    assert.equal(result.status, 0, result.stderr)
  }
  assert.equal(existsSync(f.local), false)
  assert.equal(existsSync(join(f.repo, 'target')), false)
  assert.equal(existsSync(f.notificationLog), false)
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
  assert.equal(existsSync(f.notificationLog), false)
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

test('successful installation sends one Omarchy notification only after verification', t => {
  const f = fixture(t)
  const realNotifier = readFileSync(join(f.commands, 'omarchy'), 'utf8')
  f.executable(join(f.commands, 'omarchy'), realNotifier.replace("printf '%s\\0'", `test -x "$BROWSEY_LOCAL_DIR/bin/browsey" || exit 90
cmp "$BROWSEY_LOCAL_DIR/opt/browsey/usr/bin/browsey" target/release/browsey || exit 91
printf '%s\\0'`))
  const result = f.run()
  assert.equal(result.status, 0, result.stderr)
  const args = readFileSync(f.notificationLog, 'utf8').split('\0').filter(Boolean)
  assert.deepEqual(args, ['omarchy', 'notification', 'send', '--app-name', 'Browsey',
    '-u', 'normal', '-i', 'folder', 'Browsey installed',
    'Installation completed successfully. Finish active file operations before restarting Browsey.'])
})

test('failed Omarchy notification falls back to standard desktop notifications', t => {
  const f = fixture(t)
  const result = f.run([], { BROWSEY_TEST_OMARCHY_STATUS: '1' })
  assert.equal(result.status, 0, result.stderr)
  const args = readFileSync(f.notificationLog, 'utf8').split('\0').filter(Boolean)
  assert.equal(args.filter(arg => arg === 'omarchy').length, 1)
  assert.equal(args.filter(arg => arg === 'notify-send').length, 1)
  assert.ok(args.includes('Browsey installed'))
})

test('failed final installation verification never sends a success notification', t => {
  const f = fixture(t)
  f.executable(join(f.commands, 'cmp'), `#!/bin/sh
case "$3" in */opt/browsey/usr/bin/browsey) exit 1;; esac
exec /usr/bin/cmp "$@"
`)
  assert.notEqual(f.run().status, 0)
  assert.equal(existsSync(f.notificationLog), false)
})

test('notification failures never turn a verified installation into failure', t => {
  const f = fixture(t)
  const result = f.run([], { BROWSEY_TEST_OMARCHY_STATUS: '1', BROWSEY_TEST_NOTIFY_STATUS: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /Installed:/)
  assert.match(result.stderr, /desktop notification could not be delivered/)
  assert.equal(existsSync(join(f.local, 'opt/browsey/usr/bin/browsey')), true)
})

test('notification calls are bounded and timeout does not change installation success', t => {
  const f = fixture(t)
  f.executable(join(f.commands, 'timeout'), `#!/bin/sh
test "$1" = --kill-after=1s || exit 90
test "$2" = 5s || exit 91
exit 124
`)
  const result = f.run()
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stderr, /desktop notification could not be delivered/)
  assert.equal(existsSync(f.notificationLog), false)
})

test('missing desktop notification tools do not prevent headless installation', t => {
  const f = fixture(t)
  rmSync(join(f.commands, 'omarchy'))
  rmSync(join(f.commands, 'notify-send'))
  for (const name of ['bash', 'uname', 'dirname', 'npm', 'node', 'flock', 'tar', 'install',
    'mktemp', 'realpath', 'mkdir', 'mv', 'cmp', 'cp', 'rm', 'chmod', 'timeout']) {
    const found = spawnSync('bash', ['-c', 'command -v "$1"', 'locate', name], { encoding: 'utf8' })
    assert.equal(found.status, 0, name)
    symlinkSync(found.stdout.trim(), join(f.commands, name))
  }
  const result = f.run([], { PATH: f.commands, HOME: f.root })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stderr, /desktop notification could not be delivered/)
  assert.equal(existsSync(f.notificationLog), false)
})
