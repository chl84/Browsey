import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const vendor = join(root, 'vendor')
const manifest = JSON.parse(readFileSync(join(vendor, 'provenance.json'), 'utf8'))
const files = (directory, prefix = '') => readdirSync(directory, { withFileTypes: true })
  .flatMap(entry => entry.isDirectory()
    ? files(join(directory, entry.name), join(prefix, entry.name))
    : [join(prefix, entry.name)]).sort()

for (const [name, record] of Object.entries(manifest)) {
  assert.match(record.archive_sha256, /^[a-f0-9]{64}$/)
  const directory = join(vendor, record.source_directory)
  const hash = createHash('sha256')
  for (const file of files(directory)) {
    hash.update(file).update('\0').update(readFileSync(join(directory, file))).update('\0')
  }
  assert.equal(hash.digest('hex'), record.source_tree_sha256, `${name}: source tree changed`)
  for (const [file, expected] of Object.entries(record.files ?? {})) {
    assert.equal(createHash('sha256').update(readFileSync(join(vendor, file))).digest('hex'), expected, `${file}: checksum mismatch`)
  }
}

const iterator = readFileSync(join(vendor, 'glib/src/variant_iter.rs'), 'utf8')
assert.match(iterator, /let mut p: \*mut libc::c_char/)
assert.match(iterator, /g_variant_get_child\([\s\S]*?&mut p,/)
const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8')
assert.ok(cargo.includes('glib = { path = "vendor/glib" }'))
assert.ok(readFileSync(join(vendor, 'glib/LICENSE'), 'utf8').includes('Permission is hereby granted'))
for (const file of ['LICENSE', 'COPYRIGHT']) {
  assert.equal(readFileSync(join(root, `resources/glib-${file}`), 'utf8'), readFileSync(join(vendor, `glib/${file}`), 'utf8'))
}
if (manifest.unrar_source) {
  assert.ok(cargo.includes('unrar_sys = { path = "vendor/unrar-sys" }'))
  const version = readFileSync(join(vendor, 'unrar-sys/vendor/unrar/version.hpp'), 'utf8')
  assert.match(version, /RARVER_MAJOR\s+7\b/)
  assert.match(version, /RARVER_MINOR\s+23\b/)
  assert.match(version, /RARVER_BETA\s+0\b/)
  const bindings = readFileSync(join(vendor, 'unrar-sys/src/lib.rs'), 'utf8')
  assert.equal(bindings.match(/#\[repr\(C, packed\)\]/g)?.length, 4)
  assert.equal(readFileSync(join(root, 'resources/unrar-LICENSE.txt'), 'utf8'), readFileSync(join(vendor, 'unrar-sys/vendor/unrar/license.txt'), 'utf8'))
  for (const file of ['LICENSE-MIT', 'LICENSE-APACHE']) {
    assert.equal(readFileSync(join(root, `resources/unrar-sys-${file}`), 'utf8'), readFileSync(join(vendor, `unrar-sys/${file}`), 'utf8'))
  }
}
console.log('Vendored source hashes, native UnRAR version/ABI and the GLib soundness backport verified.')
