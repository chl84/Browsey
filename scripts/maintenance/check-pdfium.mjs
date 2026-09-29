import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const resources = join(root, 'resources')
const manifest = JSON.parse(readFileSync(join(resources, 'pdfium.json'), 'utf8'))
const read = (path) => readFileSync(path, 'utf8')
const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
const [major, minor, build, patch] = manifest.version.split('.')
assert.equal(manifest.release_tag, `chromium/${build}`)
assert.ok(manifest.release_url.endsWith(`/releases/tag/${manifest.release_tag}`))

for (const pkg of manifest.packages) {
  const directory = join(resources, pkg.directory)
  for (const [file, expected] of Object.entries(pkg.files)) {
    assert.equal(sha256(join(directory, file)), expected, `${pkg.directory}/${file}: checksum mismatch`)
  }
  const version = read(join(directory, 'VERSION'))
  for (const [key, expected] of Object.entries({ MAJOR: major, MINOR: minor, BUILD: build, PATCH: patch })) {
    assert.match(version, new RegExp(`^${key}=${expected}$`, 'm'))
  }
  assert.ok(read(join(directory, 'PDFiumConfig.cmake')).includes(`set(PDFium_VERSION "${manifest.version}")`))
  const args = read(join(directory, 'args.gn'))
  for (const [key, expected] of Object.entries({ ...manifest.build, target_os: pkg.target_os })) {
    const value = typeof expected === 'string' ? `"${expected}"` : String(expected)
    assert.ok(args.includes(`${key} = ${value}`), `${pkg.directory}: unexpected ${key}`)
  }
  assert.match(read(join(directory, 'LICENSE')), /Benoit Blanchon/)
  assert.match(read(join(directory, 'licenses/pdfium.txt')), /PDFium Authors/)
  assert.match(read(join(directory, 'include/fpdfview.h')), /FPDF_InitLibrary/)
  assert.match(pkg.archive_sha256, /^[a-f0-9]{64}$/)
  assert.ok(pkg.archive_url.includes(`/releases/download/${manifest.release_tag}/`))
}

// Keep both platforms' public headers and licenses in lockstep. Ignore .orig
// files supplied by the distributor, which are not part of the packaged API.
const relativeFiles = (dir, prefix = '') => readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
  const path = join(prefix, entry.name)
  return entry.isDirectory() ? relativeFiles(join(dir, entry.name), path)
    : entry.name.endsWith('.orig') ? [] : [path]
}).sort()
for (const group of ['include', 'licenses']) {
  const linux = join(resources, 'pdfium-linux-x64', group)
  const windows = join(resources, 'pdfium-win-x64', group)
  const files = relativeFiles(linux)
  assert.deepEqual(relativeFiles(windows), files, `${group}: platform file sets differ`)
  for (const file of files) {
    assert.equal(read(join(linux, file)).replaceAll('\r\n', '\n'), read(join(windows, file)).replaceAll('\r\n', '\n'), `${group}/${file}: platform contents differ`)
  }
}

const cargo = read(join(root, 'Cargo.toml'))
const dependency = cargo.split('\n').find(line => line.startsWith('pdfium-render ='))
assert.ok(dependency?.includes(`version = "=${manifest.bindings.version}"`))
for (const feature of [manifest.bindings.api, 'thread_safe', 'image_025']) assert.ok(dependency.includes(`"${feature}"`))
assert.ok(dependency.includes('default-features = false'))
const notices = read(join(root, 'THIRD_PARTY_NOTICES'))
assert.ok(notices.includes(manifest.version))
assert.ok(notices.includes(manifest.release_url))
assert.ok(!notices.includes('pdfium-binaries/releases/latest'))
assert.match(read(join(resources, 'pdfium-render-LICENSE.md')), /Apache License.*MIT License/s)
assert.equal(sha256(join(root, 'tests/fixtures/pdf/encrypted.pdf')), 'cbbead7185d9cd46daecaf60165a9aa7b9f2daceb3623218c328cb015ae862d5')
console.log(`PDFium ${manifest.version}: binaries, headers, licenses, build flags and binding profile verified.`)
