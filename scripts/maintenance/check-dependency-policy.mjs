import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export function validateDependencyPolicy(cargo, frontend, lock) {
  const version = cargo.match(/^tauri = \{ version = "=(\d+\.\d+\.\d+)"/m)?.[1]
  assert.ok(version, 'Tauri core must be pinned to an exact stable version')
  assert.match(cargo, /^tauri-build = \{ version = "=\d+\.\d+\.\d+"/m)
  for (const name of ['@tauri-apps/api', '@tauri-apps/cli']) {
    const dependencies = name.endsWith('/api') ? 'dependencies' : 'devDependencies'
    assert.equal(frontend[dependencies][name], `=${version}`, `${name}: desktop stack mismatch`)
    assert.equal(lock.packages[''][dependencies][name], `=${version}`, `${name}: stale lockfile constraint`)
    assert.equal(lock.packages[`node_modules/${name}`].version, version, `${name}: stale installed resolution`)
  }
  assert.match(cargo, /pdfium-render = \{ version = "=\d+\.\d+\.\d+"/)
  assert.match(cargo, /^gio = "=0\.18\.4"/m)
  assert.match(cargo, /^gtk = "=0\.18\.2"/m)
  return version
}

export function validateGlibRegressionPolicy(cargo, regressionCargo) {
  assert.match(cargo, /^gio\.workspace = true$/m, 'Browsey must inherit the shared GIO pin')
  assert.match(regressionCargo, /^gio\.workspace = true$/m, 'GLib regression must inherit the shared GIO pin')
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
  const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8')
  const frontend = JSON.parse(readFileSync(join(root, 'frontend/package.json'), 'utf8'))
  const lock = JSON.parse(readFileSync(join(root, 'frontend/package-lock.json'), 'utf8'))
  const version = validateDependencyPolicy(cargo, frontend, lock)
  const regressionCargo = readFileSync(join(root, 'tests/glib-regression/Cargo.toml'), 'utf8')
  validateGlibRegressionPolicy(cargo, regressionCargo)
  console.log(`Coordinated Tauri ${version}, PDFium and GTK/GIO dependency pins verified.`)
}
