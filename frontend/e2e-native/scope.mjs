import assert from 'node:assert/strict'
import path from 'node:path'

export const kinds = ['local', 'usb', 'network', 'cloud', 'mobile']
export const folderName = 'ai_agent_testfolder'

export function validatePath(raw, cloud = raw?.startsWith('rclone://')) {
  assert.equal(typeof raw, 'string', 'A full test path is required')
  assert.ok(raw && !/[\\\0]/.test(raw) && !raw.endsWith('/'), 'No aliases, NUL, backslashes or trailing slash')
  const tail = cloud ? raw.slice('rclone://'.length) : raw.slice(1)
  assert.ok(cloud ? raw.startsWith('rclone://') : raw.startsWith('/'), 'Use an absolute path, or rclone:// for cloud')
  const parts = tail.split('/')
  assert.ok(parts.every(part => part && part !== '.' && part !== '..'), 'No relative/empty path components')
  if (cloud) assert.match(parts[0], /^[a-zA-Z0-9_ -]+$/, 'Invalid rclone remote')
  return parts
}

export function validateConfig(value) {
  assert.equal(value.schema, 1, 'Expected native config schema 1')
  assert.ok(value.targets && typeof value.targets === 'object', 'Explicit targets are required')
  assert.ok(Object.keys(value.targets).every(kind => kinds.includes(kind)), 'Unknown target kind')
  const targets = kinds.filter(kind => value.targets[kind]).map(kind => {
    const raw = value.targets[kind]
    const parts = validatePath(raw, kind === 'cloud')
    assert.equal(parts.at(-1), folderName, 'Only an existing folder named exactly ai_agent_testfolder may be approved')
    return { kind, path: raw }
  })
  assert.ok(targets.some(target => target.kind === 'local'), 'An approved local test folder is required')
  assert.equal(new Set(targets.map(target => target.path)).size, targets.length, 'Do not alias the same test root')
  for (const a of targets) for (const b of targets) {
    if (a !== b) assert.ok(!inside(a.path, b.path), 'Test roots cannot contain each other')
  }
  const matrix = value.matrix ?? 'local-hub'
  assert.ok(['local-hub', 'all-pairs'].includes(matrix), 'Invalid transfer matrix')
  if (targets.some(target => target.kind === 'cloud')) {
    validatePath(value.rcloneConfig, false)
    const local = targets.find(target => target.kind === 'local')
    assert.ok(inside(local.path, value.rcloneConfig) && value.rcloneConfig !== local.path,
      'An explicitly approved test-only rclone config must be inside the local ai_agent_testfolder')
  }
  return { targets, matrix, rcloneConfig: value.rcloneConfig ?? null }
}

export function inside(root, candidate) {
  return candidate === root || candidate.startsWith(`${root}/`)
}

export function ownedPath(roots, candidate) {
  validatePath(candidate)
  assert.ok(roots.some(root => inside(root, candidate)), 'Refusing access outside this owned native run')
  return candidate
}

export function child(root, leaf) {
  assert.ok(leaf && !/[/\\\0]/.test(leaf) && leaf !== '.' && leaf !== '..', 'Expected one safe path component')
  return `${root}/${leaf}`
}

export function makePlan(config, runId) {
  assert.match(runId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
  const targets = config.targets.map(target => {
    // Compact full-entropy identifier leaves room for Unix-domain socket names.
    const run = child(target.path, `.bnt-${runId.replaceAll('-', '')}`)
    return { ...target, run, files: child(run, 'files') }
  })
  const local = targets.find(target => target.kind === 'local')
  assert.ok(Buffer.byteLength(`${local.run}/r/browsey-rclone-rc/rcd-4194304.sock`) < 108,
    'Approved local root is too long for isolated native Unix sockets')
  const routes = []
  for (const from of targets) for (const to of targets) {
    if (from === to || (config.matrix === 'local-hub' && from.kind !== 'local' && to.kind !== 'local')) continue
    routes.push({ from: from.kind, to: to.kind })
  }
  return { runId, targets, routes, notConfigured: kinds.filter(kind => !targets.some(target => target.kind === kind)) }
}

// Metadata probes inspect only the exact requested path/components; never discover
// test folders by listing mounts, parents, home directories, accounts or devices.
export async function noLinks(raw, fs) {
  validatePath(raw, false)
  let current = '/'
  for (const component of raw.slice(1).split('/')) {
    current = path.join(current, component)
    let stat
    try { stat = await fs.lstat(current) } catch (error) {
      if (error.code === 'ENOENT') return
      throw error
    }
    assert.ok(!stat.isSymbolicLink(), 'Native test paths cannot traverse symlinks')
  }
}

export function candidateEnvironment(host, profile, session) {
  // Do not inherit personal credentials, tool overrides, NODE_OPTIONS or SSH agents.
  const env = { PATH: '/usr/bin:/bin', LANG: host.LANG || 'C.UTF-8', HOME: `${profile}/home`,
    XDG_DATA_HOME: `${profile}/data`, XDG_CONFIG_HOME: `${profile}/config`,
    XDG_CACHE_HOME: `${profile}/cache`, XDG_STATE_HOME: `${profile}/state`,
    XDG_RUNTIME_DIR: `${path.dirname(profile)}/r`, TMPDIR: `${path.dirname(profile)}/t`,
    RCLONE_CONFIG: `${profile}/config/rclone.conf`, RUST_LOG: 'warn', NO_AT_BRIDGE: '0',
    BROWSEY_UNDO_DIR: `${profile}/data/browsey/undo-sessions`,
    BROWSEY_NATIVE_TEST_SESSION: JSON.stringify(session), TAURI_WEBVIEW_AUTOMATION: 'true' }
  for (const key of ['DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'XDG_SESSION_TYPE', 'XDG_CURRENT_DESKTOP']) {
    if (host[key]) env[key] = host[key]
  }
  if (host.WAYLAND_DISPLAY) env.WAYLAND_DISPLAY = host.WAYLAND_DISPLAY.startsWith('/')
    ? host.WAYLAND_DISPLAY : path.join(host.XDG_RUNTIME_DIR, host.WAYLAND_DISPLAY)
  return env
}
