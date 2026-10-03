import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { docsPages } from '../src/content/pages.ts'
import { canonicalHash, filterPages, inlineParts, normalizeHash, sectionMatches } from '../src/lib/docs.ts'
import { DEFAULT_SETTINGS } from '../../frontend/src/features/settings/settingsTypes.ts'

const root = fileURLToPath(new URL('../../', import.meta.url))
const page = (id) => docsPages.find((item) => item.id === id)
const section = (pageId, sectionId) => page(pageId).sections.find((item) => item.id === sectionId)
const text = (item) => [item.body, item.note, item.code, ...(item.bullets ?? [])].join(' ')

test('page/section IDs are unique and every deep link round-trips', () => {
  assert.equal(new Set(docsPages.map((item) => item.id)).size, docsPages.length)
  for (const item of docsPages) {
    assert.match(item.id, /^[a-z0-9-]+$/)
    assert.ok(item.title && item.summary && item.sections.length)
    assert.equal(new Set(item.sections.map((entry) => entry.id)).size, item.sections.length)
    assert.deepEqual(normalizeHash(canonicalHash(item.id), docsPages), { pageId: item.id, sectionId: '' })
    for (const entry of item.sections) {
      assert.match(entry.id, /^[a-z0-9-]+$/)
      assert.ok(entry.title)
      assert.deepEqual(normalizeHash(canonicalHash(item.id, entry.id), docsPages), { pageId: item.id, sectionId: entry.id })
    }
  }
})

for (const hash of ['', '#', '#/', '#/missing', '#/%E0%A4%A', '#/overview/missing', '#overview']) {
  test(`normalizes empty, legacy or invalid route ${JSON.stringify(hash)}`, () => {
    assert.deepEqual(normalizeHash(hash, docsPages), { pageId: 'overview', sectionId: '' })
  })
}

test('decodes encoded route components safely', () => {
  assert.deepEqual(normalizeHash('#/%6Fverview/%73tatus', docsPages), { pageId: 'overview', sectionId: 'status' })
  assert.equal(canonicalHash('a b', 'x/y'), '#/a%20b/x%2Fy')
})

test('search is case-insensitive, whitespace-tolerant and matches multiple terms', () => {
  assert.equal(filterPages(docsPages, '  ').length, docsPages.length)
  assert.ok(filterPages(docsPages, ' PASSWORD  RAR ').some((item) => item.id === 'user-workflows'))
  assert.ok(sectionMatches(section('user-workflows', 'archives-flow'), 'password rar'))
  assert.ok(filterPages(docsPages, 'Version bumps and publication').some((item) => item.id === 'development'))
  assert.deepEqual(filterPages(docsPages, 'no-matching-docs-xyz'), [])
})

test('inline code is segmented without interpreting HTML or unmatched backticks', () => {
  assert.deepEqual(inlineParts('Use `rclone config` now.'), [
    { code: false, text: 'Use ' }, { code: true, text: 'rclone config' }, { code: false, text: ' now.' },
  ])
  assert.deepEqual(inlineParts('<script>alert(1)</script>'), [{ code: false, text: '<script>alert(1)</script>' }])
  assert.deepEqual(inlineParts('a `broken'), [{ code: false, text: 'a `broken' }])
  assert.deepEqual(inlineParts(''), [])
})

test('all structured links have valid routes or existing repository files/anchors', () => {
  const headings = (content) => [...content.matchAll(/^#{1,6} (.+)$/gm)]
    .map((match) => match[1].toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, '').trim().replace(/\s/g, '-'))
  for (const item of docsPages) for (const entry of item.sections) for (const link of entry.links ?? []) {
    assert.ok(link.label)
    if (link.href.startsWith('#/')) {
      const normalized = normalizeHash(link.href, docsPages)
      assert.equal(canonicalHash(normalized.pageId, normalized.sectionId), link.href)
      continue
    }
    const url = new URL(link.href)
    assert.equal(url.protocol, 'https:')
    const repo = url.pathname.match(/^\/chl84\/Browsey\/(?:blob|tree)\/main\/(.+)$/)
    if (!repo || url.hostname !== 'github.com') continue
    const file = resolve(root, decodeURIComponent(repo[1]))
    assert.ok(existsSync(file), `Missing linked repository target: ${link.href}`)
    if (url.hash) assert.ok(headings(readFileSync(file, 'utf8')).includes(url.hash.slice(1)), `Missing anchor: ${link.href}`)
  }
})

test('current version, release and source requirements stay aligned', () => {
  const version = readFileSync(resolve(root, 'Cargo.toml'), 'utf8').match(/^version = "([^"]+)"$/m)[1]
  assert.ok(text(section('overview', 'status')).includes(`Browsey ${version} `))
  assert.ok(page('release-notes').sections.some((entry) => entry.title.startsWith(`v${version} (`)))
  for (const id of ['run-dev', 'build-artifacts']) assert.ok(text(section('getting-started', id)).includes('frontend/node_modules/.bin/tauri'))
  assert.doesNotMatch(section('getting-started', 'run-dev').code, /cargo tauri|npm .* install/)
  assert.match(text(section('getting-started', 'requirements-common')), /not required to run/)
})

test('archive/password and cloud-cache guidance does not regress to obsolete claims', () => {
  assert.match(text(section('user-workflows', 'archives-flow')), /encrypted ZIP, 7z and RAR/)
  assert.doesNotMatch(text(section('user-workflows', 'archives-flow')), /password-protected archives report explicit errors/)
  assert.match(text(section('settings-shortcuts', 'settings-data-actions')), /not persistent working copies/)
  assert.match(text(section('known-limitations', 'undo-lifecycle')), /retains live or recovery-marked/)
  assert.match(text(section('getting-started', 'cloud-rclone-ops-model')), /never automatically replayed/)
  assert.doesNotMatch(text(section('getting-started', 'cloud-rclone-ops-model')), /current main \(Unreleased\)/)
})

test('Properties and Open With describe the actual modal controls', () => {
  assert.match(text(section('user-workflows', 'properties-flow')), /Ownership tab edits owner\/group/)
  assert.doesNotMatch(text(section('known-limitations', 'open-with-limitations')), /Custom command launching is supported/)
  assert.match(text(section('known-limitations', 'open-with-limitations')), /does not provide an arbitrary command-entry field/)
})

test('historical release versions and dates agree with the changelog', () => {
  const changelog = readFileSync(resolve(root, 'CHANGELOG.md'), 'utf8')
  for (const entry of page('release-notes').sections) {
    if (entry.id === 'unreleased') continue
    const [, version, date] = entry.title.match(/^v(.+) \((\d{4}-\d{2}-\d{2})\)$/)
    assert.ok(changelog.includes(`## v${version} — ${date}`), `Release date differs: ${entry.title}`)
  }
})

test('documented Settings defaults agree with the application defaults', () => {
  const defaults = section('settings-shortcuts', 'default-settings').bullets
  const enabled = (value) => value ? 'enabled' : 'disabled'
  const expected = [
    `Start directory: ${DEFAULT_SETTINGS.startDir}`,
    `Default view: ${DEFAULT_SETTINGS.defaultView}`,
    `Density: ${DEFAULT_SETTINGS.density}`,
    `Theme: ${DEFAULT_SETTINGS.themeMode}`,
    `Show hidden: ${enabled(DEFAULT_SETTINGS.showHidden)}`,
    `Hidden files last: ${enabled(DEFAULT_SETTINGS.hiddenFilesLast)}`,
    `Folders first: ${enabled(DEFAULT_SETTINGS.foldersFirst)}`,
    `Confirm permanent delete: ${enabled(DEFAULT_SETTINGS.confirmDelete)}`,
    `Archive name: ${DEFAULT_SETTINGS.archiveName} (.zip)`,
    `Archive level: ${DEFAULT_SETTINGS.archiveLevel}`,
    `Open destination after extract: ${enabled(DEFAULT_SETTINGS.openDestAfterExtract)}`,
    `Video thumbnails: ${enabled(DEFAULT_SETTINGS.videoThumbs)}`,
    `Cloud thumbnails: ${enabled(DEFAULT_SETTINGS.cloudThumbs)}`,
    `Cloud integration: ${enabled(DEFAULT_SETTINGS.cloudEnabled)}`,
    `Thumbnail cache: ${DEFAULT_SETTINGS.thumbCacheMb} MB`,
    `Mount polling: ${DEFAULT_SETTINGS.mountsPollMs} ms`,
    `Double-click speed: ${DEFAULT_SETTINGS.doubleClickMs} ms`,
    `Hardware acceleration: ${enabled(DEFAULT_SETTINGS.hardwareAcceleration)}`,
    `High contrast: ${enabled(DEFAULT_SETTINGS.highContrast)}`,
    `Scrollbar width: ${DEFAULT_SETTINGS.scrollbarWidth} px`,
    `Log level: ${DEFAULT_SETTINGS.logLevel}`,
  ]
  for (const value of expected) assert.ok(defaults.includes(value), `Missing/outdated default: ${value}`)
})
