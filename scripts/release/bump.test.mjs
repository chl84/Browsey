import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { applyPlan, buildPlan, parseArgs, versionFiles } from './bump.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const source = Object.fromEntries(versionFiles.map((path) => [path, readFileSync(join(root, path), 'utf8')]))
// Keep this suite runnable immediately after a real bump, when Unreleased is empty.
source['CHANGELOG.md'] = source['CHANGELOG.md'].replace(
  /## Unreleased\n[\s\S]*?(?=## v)/, '## Unreleased\n\n- Release-tool fixture change.\n\n')
const currentVersion = source['Cargo.toml'].match(/^version = "([^"]+)"$/m)[1]
const [major, minor, patch] = currentVersion.split('.').map(BigInt)
const nextVersion = `${major}.${minor}.${patch + 1n}`
const currentDate = source['packaging/com.browsey.metainfo.xml'].match(/<release version="[^"]+" date="([^"]+)"/)[1]
const date = new Date(`${currentDate}T00:00:00Z`)
date.setUTCDate(date.getUTCDate() + 1)
const nextDate = date.toISOString().slice(0, 10)

function git(directory, args) {
  const result = spawnSync('git', args, { cwd: directory, encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  return result.stdout
}

function put(directory, path, content) {
  mkdirSync(dirname(join(directory, path)), { recursive: true })
  writeFileSync(join(directory, path), content)
}

function commit(directory) {
  git(directory, ['add', '.'])
  git(directory, ['-c', 'user.name=Release Test', '-c', 'user.email=test@example.invalid',
    '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', 'commit', '--quiet', '-m', 'fixture'])
}

function fixture(t, extra = {}) {
  const directory = mkdtempSync(join(tmpdir(), 'browsey-bump-test-'))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  git(directory, ['init', '--quiet'])
  for (const [path, content] of Object.entries({
    ...source,
    'tauri.conf.json': readFileSync(join(root, 'tauri.conf.json'), 'utf8'),
    'scripts/release/bump.mjs': readFileSync(join(root, 'scripts/release/bump.mjs'), 'utf8'),
    [`docs/releases/${currentVersion}.md`]: 'Historical release notes. Do not change.\n',
    'frontend/package.json': '{"version":"0.0.0"}\n',
    'docs-site/package.json': '{"version":"0.1.0"}\n',
    ...extra,
  })) put(directory, path, content)
  commit(directory)
  return directory
}

function cli(directory, args, env = {}) {
  return spawnSync(process.execPath, ['scripts/release/bump.mjs', ...args], {
    cwd: directory, encoding: 'utf8', env: { ...process.env, ...env },
  })
}

function changed(plan, path) {
  return plan.changes.find((change) => change.path === path).after
}

test('coordinates exactly the app fields and creates honest release preparation notes', () => {
  const plan = buildPlan(source, nextVersion, nextDate)
  assert.equal(plan.changes.length, 8)
  assert.equal(plan.oldVersion, currentVersion)
  assert.equal(changed(plan, 'Cargo.toml').replace(`version = "${nextVersion}"`, `version = "${currentVersion}"`), source['Cargo.toml'])
  assert.equal(changed(plan, 'Cargo.lock').replace(`name = "browsey"\nversion = "${nextVersion}"`,
    `name = "browsey"\nversion = "${currentVersion}"`), source['Cargo.lock'])
  assert.ok(changed(plan, 'README.md').includes(`releases/tag/v${nextVersion}`))
  assert.ok(changed(plan, 'README.md').includes(`Browsey \`${nextVersion}\` is Linux-first.`))
  assert.ok(changed(plan, 'docs-site/src/content/pages.ts').includes(`title: 'v${nextVersion} (${nextDate})'`))
  assert.ok(changed(plan, 'packaging/com.browsey.metainfo.xml').includes(`<release version="${nextVersion}" date="${nextDate}"/>`))
  const originalHistory = source['CHANGELOG.md'].slice(source['CHANGELOG.md'].indexOf(`## v${currentVersion} `))
  assert.ok(changed(plan, 'CHANGELOG.md').endsWith(originalHistory))
  const originalRpmHistory = source['packaging/rpm/browsey.spec'].split('%changelog\n')[1]
  assert.ok(changed(plan, 'packaging/rpm/browsey.spec').endsWith(originalRpmHistory))
  const originalSiteHistory = source['docs-site/src/content/pages.ts'].slice(
    source['docs-site/src/content/pages.ts'].indexOf(`        title: 'v${currentVersion} (`))
  assert.ok(changed(plan, 'docs-site/src/content/pages.ts').endsWith(originalSiteHistory))
  const notes = changed(plan, `docs/releases/${nextVersion}.md`)
  assert.match(notes, /not proof of testing or publication/)
  assert.match(notes, /\[ \] Review security/)
  assert.ok(notes.includes(`annotated tag \`v${nextVersion}\``))
  assert.match(notes, /Record fresh manual checks/)
  assert.ok(!plan.changes.some(({ path }) => path.endsWith('package.json')))
})

test('does not rewrite an unrelated lockfile dependency at the old app version', () => {
  const files = { ...source, 'Cargo.lock': `${source['Cargo.lock']}\n[[package]]\nname = "unrelated"\nversion = "${currentVersion}"\n` }
  assert.ok(changed(buildPlan(files, nextVersion, nextDate), 'Cargo.lock').endsWith(`name = "unrelated"\nversion = "${currentVersion}"\n`))
})

test('flattens Markdown continuations and safely escapes docs-site strings', () => {
  const notes = '- Handle "quotes", \'apostrophes\', backticks `x` and \\paths\n  without losing the continuation.\n'
  const files = { ...source, 'CHANGELOG.md': source['CHANGELOG.md'].replace(
    /## Unreleased\n[\s\S]*?(?=## v)/, `## Unreleased\n\n${notes}\n`) }
  const plan = buildPlan(files, nextVersion, nextDate)
  const expected = JSON.stringify('Handle "quotes", \'apostrophes\', backticks `x` and \\paths without losing the continuation.')
  assert.ok(changed(plan, 'docs-site/src/content/pages.ts').includes(expected))
  const js = changed(plan, 'docs-site/src/content/pages.ts').slice(
    changed(plan, 'docs-site/src/content/pages.ts').indexOf('export const docsPages: DocPage[] = '))
    .split('\nexport const docsPageMap')[0]
    .replace('export const docsPages: DocPage[] = ', 'return ')
  const release = new Function(js)().find((page) => page.id === 'release-notes')
  assert.deepEqual(release.sections.find((section) => section.title === `v${nextVersion} (${nextDate})`).bullets,
    ['Handle "quotes", \'apostrophes\', backticks `x` and \\paths without losing the continuation.'])
  assert.ok(changed(plan, `docs/releases/${nextVersion}.md`).includes(notes.trim()))
})

for (const invalid of [currentVersion, '0.0.0', '01.2.3', 'v2.0.0', '2.0.0-rc.1', '2.0', '../../oops']) {
  test(`rejects invalid/non-increasing version ${invalid}`, () => {
    assert.throws(() => buildPlan(source, invalid, nextDate), /version|greater/i)
  })
}

test('accepts minor and major increases without modifying dependency versions', () => {
  for (const version of [`${major}.${minor + 1n}.0`, `${major + 1n}.0.0`]) {
    assert.equal(buildPlan(source, version, nextDate).version, version)
  }
})

for (const invalidDate of ['2026-02-30', '2026-13-01', '26-09-30', '2000-01-01']) {
  test(`rejects invalid/backdated release date ${invalidDate}`, () => {
    assert.throws(() => buildPlan(source, nextVersion, invalidDate), /date/i)
  })
}

test('rejects divergent versions, duplicate fields, missing anchors and populated site Unreleased', () => {
  const cases = [
    ['Cargo.lock', `name = "browsey"\nversion = "${currentVersion}"`, 'name = "browsey"\nversion = "0.0.0"'],
    ['packaging/rpm/browsey.spec', `Version:        ${currentVersion}`, 'Version:        0.0.0'],
    ['packaging/com.browsey.metainfo.xml', `<release version="${currentVersion}"`, '<release version="0.0.0"'],
    ['CHANGELOG.md', `## v${currentVersion} `, '## v0.0.0 '],
    ['README.md', 'Downloads:', 'Old downloads:'],
    ['README.md', '## Status', '## Old status'],
    ['README.md', '## Status', 'Downloads: duplicate\n\n## Status'],
    ['docs-site/src/content/pages.ts', `Browsey ${currentVersion} `, 'Browsey 0.0.0 '],
    ['docs-site/src/content/pages.ts', "title: 'Unreleased',\n        bullets: []", "title: 'Unreleased',\n        bullets: ['Needs review']"],
  ]
  for (const [path, from, to] of cases) {
    assert.ok(source[path].includes(from), `Fixture anchor not found: ${from}`)
    assert.throws(() => buildPlan({ ...source, [path]: source[path].replace(from, to) }, nextVersion, nextDate),
      undefined, `Should reject changed ${path}`)
  }
})

test('refuses a bump without Unreleased changes', () => {
  const files = { ...source, 'CHANGELOG.md': source['CHANGELOG.md'].replace(/## Unreleased\n[\s\S]*?(?=## v)/, '## Unreleased\n\n') }
  assert.throws(() => buildPlan(files, nextVersion, nextDate), /at least one change/)
})

test('parses safe CLI options and requires explicit apply for verification', () => {
  const args = parseArgs([nextVersion, '--date', nextDate, '--apply', '--verify'])
  assert.equal(args.date, nextDate)
  assert.equal(args.apply, true)
  assert.equal(args.verify, true)
  assert.equal(parseArgs([nextVersion]).apply, false)
  for (const invalid of [[], [nextVersion, '--verify'], [nextVersion, '--date'],
    [nextVersion, '--push'], [nextVersion, '--apply', '--apply'], [nextVersion, nextVersion]]) {
    assert.throws(() => parseArgs(invalid))
  }
})

test('CLI preview and help never change files, even with pending edits', (t) => {
  const directory = fixture(t)
  put(directory, 'pending.txt', 'User changes.\n')
  const before = git(directory, ['status', '--porcelain'])
  const result = cli(directory, [nextVersion, '--date', nextDate])
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /Preview only/)
  assert.equal(git(directory, ['status', '--porcelain']), before)
  for (const path of versionFiles) assert.equal(readFileSync(join(directory, path), 'utf8'), source[path])
  assert.equal(cli(directory, ['--help']).status, 0)
  assert.equal(git(directory, ['status', '--porcelain']), before)
})

test('CLI applies the exact plan without modifying history, HEAD, index or tags', (t) => {
  const directory = fixture(t)
  const head = git(directory, ['rev-parse', 'HEAD'])
  const result = cli(directory, [nextVersion, '--date', nextDate, '--apply'])
  assert.equal(result.status, 0, result.stderr)
  for (const change of buildPlan(source, nextVersion, nextDate).changes) {
    assert.equal(readFileSync(join(directory, change.path), 'utf8'), change.after)
  }
  assert.equal(git(directory, ['rev-parse', 'HEAD']), head)
  assert.equal(git(directory, ['diff', '--cached']), '')
  assert.equal(git(directory, ['tag', '--list']), '')
  assert.equal(readFileSync(join(directory, `docs/releases/${currentVersion}.md`), 'utf8'), 'Historical release notes. Do not change.\n')
  assert.equal(readFileSync(join(directory, 'frontend/package.json'), 'utf8'), '{"version":"0.0.0"}\n')
  assert.equal(readFileSync(join(directory, 'docs-site/package.json'), 'utf8'), '{"version":"0.1.0"}\n')
  git(directory, ['diff', '--check'])
})

test('apply rejects dirty tracked, staged and untracked work without losing it', (t) => {
  for (const mode of ['tracked', 'staged', 'untracked']) {
    const directory = fixture(t)
    put(directory, mode === 'untracked' ? 'pending.txt' : 'README.md', 'User changes.\n')
    if (mode === 'staged') git(directory, ['add', 'README.md'])
    const before = git(directory, ['status', '--porcelain'])
    assert.throws(() => applyPlan(directory, buildPlan(source, nextVersion, nextDate)), /not clean/)
    const result = cli(directory, [nextVersion, '--date', nextDate, '--apply'])
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /not clean/)
    assert.equal(git(directory, ['status', '--porcelain']), before)
    assert.equal(readFileSync(join(directory, mode === 'untracked' ? 'pending.txt' : 'README.md'), 'utf8'), 'User changes.\n')
    assert.equal(readFileSync(join(directory, 'Cargo.toml'), 'utf8'), source['Cargo.toml'])
  }
})

test('CLI rejects an existing note, local tag, remote tag or explicit Tauri version', (t) => {
  for (const collision of ['note', 'local tag', 'origin tag', 'tauri']) {
    const directory = fixture(t)
    if (collision === 'note') put(directory, `docs/releases/${nextVersion}.md`, 'Already exists.\n')
    if (collision === 'local tag') git(directory, ['tag', `v${nextVersion}`])
    if (collision === 'origin tag') {
      const remote = fixture(t)
      git(remote, ['tag', `v${nextVersion}`])
      git(directory, ['remote', 'add', 'origin', remote])
    }
    if (collision === 'tauri') put(directory, 'tauri.conf.json', '{"version":"0.0.0"}\n')
    const result = cli(directory, [nextVersion, '--date', nextDate])
    assert.notEqual(result.status, 0, `Should reject ${collision}`)
    assert.match(result.stderr, /already exist|explicit version/)
    assert.equal(readFileSync(join(directory, 'Cargo.toml'), 'utf8'), source['Cargo.toml'])
  }
})

test('CLI fails closed if origin cannot be queried', (t) => {
  const directory = fixture(t)
  git(directory, ['remote', 'add', 'origin', join(directory, 'missing-remote')])
  const result = cli(directory, [nextVersion, '--date', nextDate, '--apply'])
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /ls-remote failed/)
  assert.equal(git(directory, ['status', '--porcelain']), '')
})

test('apply detects changed snapshots even if the change was committed after planning', (t) => {
  const directory = fixture(t)
  const plan = buildPlan(source, nextVersion, nextDate)
  put(directory, 'README.md', `${source['README.md']}\nCommitted later.\n`)
  commit(directory)
  assert.throws(() => applyPlan(directory, plan), /changed after planning/)
  assert.equal(git(directory, ['status', '--porcelain']), '')
})

test('Git checks the whole patch before changing any file', (t) => {
  const directory = fixture(t)
  const plan = buildPlan(source, nextVersion, nextDate)
  // An invalid final hunk must not leave earlier version edits behind.
  plan.changes.at(-1).after = 'Invalid trailing whitespace. \n'
  assert.throws(() => applyPlan(directory, plan), /apply failed/)
  assert.equal(git(directory, ['status', '--porcelain']), '')
})

test('verification failure remains a visible uncommitted bump, never a release', (t) => {
  const directory = fixture(t, { 'scripts/maintenance/test-both.sh': '#!/usr/bin/env bash\nexit 7\n' })
  const result = cli(directory, [nextVersion, '--date', nextDate, '--apply', '--verify'])
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /failed\. Bump remains for inspection; not tagged or published/)
  assert.match(readFileSync(join(directory, 'Cargo.toml'), 'utf8'), new RegExp(`version = "${nextVersion.replaceAll('.', '\\.')}"`))
  assert.equal(git(directory, ['tag', '--list']), '')
  assert.equal(git(directory, ['diff', '--cached']), '')
})

test('optional verification runs the strict maintenance suite and all docs gates in order', (t) => {
  const directory = fixture(t, {
    'scripts/maintenance/test-both.sh': '#!/usr/bin/env bash\ntest "$1" = "--strict-docs" || exit 9\nprintf "maintenance\\n" >> "$BUMP_VERIFY_TRACE"\n',
    '.test-bin/npm': '#!/usr/bin/env node\nrequire("node:fs").appendFileSync(process.env.BUMP_VERIFY_TRACE, process.argv.slice(2).join(" ") + "\\n")\n',
  })
  chmodSync(join(directory, '.test-bin/npm'), 0o755)
  commit(directory)
  const trace = join(directory, 'verification.log')
  const result = cli(directory, [nextVersion, '--date', nextDate, '--apply', '--verify'], {
    PATH: `${join(directory, '.test-bin')}:${process.env.PATH}`,
    BUMP_VERIFY_TRACE: trace,
  })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(readFileSync(trace, 'utf8'), 'maintenance\n--prefix docs-site run lint\n--prefix docs-site run check\n--prefix docs-site run build\n')
  assert.equal(git(directory, ['tag', '--list']), '')
})

test('supports consecutive releases without replacing the first release or its notes', (t) => {
  const directory = fixture(t)
  assert.equal(cli(directory, [nextVersion, '--date', nextDate, '--apply']).status, 0)
  commit(directory)
  const secondVersion = `${major}.${minor}.${patch + 2n}`
  const empty = cli(directory, [secondVersion, '--date', nextDate])
  assert.notEqual(empty.status, 0)
  assert.match(empty.stderr, /at least one change/)
  const firstNotes = readFileSync(join(directory, `docs/releases/${nextVersion}.md`), 'utf8')
  const changelog = readFileSync(join(directory, 'CHANGELOG.md'), 'utf8')
  put(directory, 'CHANGELOG.md', changelog.replace('## Unreleased\n\n', '## Unreleased\n\n- Second release change.\n\n'))
  commit(directory)
  const second = cli(directory, [secondVersion, '--date', nextDate, '--apply'])
  assert.equal(second.status, 0, second.stderr)
  assert.equal(readFileSync(join(directory, `docs/releases/${nextVersion}.md`), 'utf8'), firstNotes)
  assert.ok(readFileSync(join(directory, 'CHANGELOG.md'), 'utf8').includes(`## v${nextVersion} `))
  assert.ok(readFileSync(join(directory, 'docs-site/src/content/pages.ts'), 'utf8').includes(`title: 'v${nextVersion} (`))
  assert.ok(readFileSync(join(directory, 'packaging/com.browsey.metainfo.xml'), 'utf8').includes(`<release version="${nextVersion}"`))
})
