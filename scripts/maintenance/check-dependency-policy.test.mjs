import assert from 'node:assert/strict'
import test from 'node:test'
import { validateDependencyPolicy } from './check-dependency-policy.mjs'

const cargo = `tauri = { version = "=2.12.0", features = [] }
tauri-build = { version = "=2.7.0", features = [] }
pdfium-render = { version = "=0.9.4", features = [] }
gio = "=0.18.4"
gtk = "=0.18.2"`
const frontend = {
  dependencies: { '@tauri-apps/api': '=2.12.0' },
  devDependencies: { '@tauri-apps/cli': '=2.12.0' },
}
const lock = {
  packages: {
    '': structuredClone(frontend),
    'node_modules/@tauri-apps/api': { version: '2.12.0' },
    'node_modules/@tauri-apps/cli': { version: '2.12.0' },
  },
}

test('accepts a coordinated stack without conflating the tauri-build version', () => {
  assert.equal(validateDependencyPolicy(cargo, frontend, lock), '2.12.0')
})

test('rejects mismatched API/CLI and stale lockfile constraints/resolutions', () => {
  for (const name of ['@tauri-apps/api', '@tauri-apps/cli']) {
    const dependencies = name.endsWith('/api') ? 'dependencies' : 'devDependencies'
    const mismatched = structuredClone(frontend)
    mismatched[dependencies][name] = '=2.11.0'
    assert.throws(() => validateDependencyPolicy(cargo, mismatched, lock), /desktop stack mismatch/)
    const staleConstraint = structuredClone(lock)
    staleConstraint.packages[''][dependencies][name] = '^2.12.0'
    assert.throws(() => validateDependencyPolicy(cargo, frontend, staleConstraint), /stale lockfile constraint/)
    const staleResolution = structuredClone(lock)
    staleResolution.packages[`node_modules/${name}`].version = '2.11.0'
    assert.throws(() => validateDependencyPolicy(cargo, frontend, staleResolution), /stale installed resolution/)
  }
})

test('rejects unpinned desktop/native dependencies and independent GTK ABI upgrades', () => {
  for (const [from, to] of [
    ['"=2.12.0"', '"2.12.0"'], ['"=2.7.0"', '"2.7.0"'],
    ['"=0.9.4"', '"0.9.4"'], ['"=0.18.4"', '"=0.21.0"'],
    ['"=0.18.2"', '"=0.21.0"'],
  ]) {
    assert.throws(() => validateDependencyPolicy(cargo.replace(from, to), frontend, lock))
  }
})
