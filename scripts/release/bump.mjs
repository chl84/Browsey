import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
export const versionFiles = [
  'Cargo.toml',
  'Cargo.lock',
  'packaging/rpm/browsey.spec',
  'packaging/com.browsey.metainfo.xml',
  'README.md',
  'CHANGELOG.md',
  'docs-site/src/content/pages.ts',
]

function requireCondition(condition, message) {
  if (!condition) throw new Error(message)
}

function replaceOnce(text, pattern, replacement, label) {
  const matches = [...text.matchAll(new RegExp(pattern.source, `${pattern.flags.replace('g', '')}g`))]
  requireCondition(matches.length === 1, `${label}: expected exactly one matching field, found ${matches.length}`)
  return text.replace(pattern, replacement)
}

function versionParts(version) {
  requireCondition(typeof version === 'string' && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version),
    `Invalid stable version: ${version}. Use X.Y.Z without a v prefix or leading zeroes.`)
  return version.split('.').map(BigInt)
}

function releaseDate(date) {
  requireCondition(typeof date === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(date), 'Date must be YYYY-MM-DD.')
  const parsed = new Date(`${date}T00:00:00Z`)
  requireCondition(!Number.isNaN(parsed.valueOf()) && parsed.toISOString().slice(0, 10) === date,
    `Invalid release date: ${date}`)
  return parsed
}

function replaceVersion(text, oldVersion, version, label) {
  const escaped = oldVersion.replaceAll('.', '\\.')
  const field = new RegExp(`(?<![\\d.])${escaped}(?!\\d|\\.\\d)`, 'g')
  requireCondition(text.match(field), `${label}: missing current version ${oldVersion}`)
  return text.replace(field, version)
}

// Only explicitly scoped current-version fields are rewritten. Dependency pins and
// previous release entries must remain byte-for-byte unchanged.
export function buildPlan(files, version, date) {
  const next = versionParts(version)
  const parsedDate = releaseDate(date)
  const cargo = files['Cargo.toml']
  const packageSection = cargo.match(/^\[package\]\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1]
  requireCondition(packageSection && /^name = "browsey"$/m.test(packageSection), 'Missing Browsey [package] section.')
  const oldVersion = packageSection.match(/^version = "([^"]+)"$/m)?.[1]
  const previous = versionParts(oldVersion)
  const difference = next.findIndex((part, index) => part !== previous[index])
  requireCondition(difference >= 0 && next[difference] > previous[difference],
    `New version ${version} must be greater than ${oldVersion}.`)
  const changes = []
  const add = (path, after) => changes.push({ path, before: files[path] ?? null, after })
  add('Cargo.toml', replaceOnce(cargo, /^\[package\]\n[\s\S]*?(?=^\[|$(?![\s\S]))/m,
    (section) => replaceOnce(section, /^version = "[^"]+"$/m, `version = "${version}"`, 'Cargo package version'), 'Cargo package'))
  add('Cargo.lock', replaceOnce(files['Cargo.lock'], /^\[\[package\]\]\nname = "browsey"\nversion = "([^"]+)"$/m,
    (entry, lockedVersion) => {
      requireCondition(lockedVersion === oldVersion, 'Cargo.lock Browsey version differs from Cargo.toml.')
      return entry.replace(`version = "${oldVersion}"`, `version = "${version}"`)
    }, 'Cargo.lock Browsey entry'))

  const spec = replaceOnce(files['packaging/rpm/browsey.spec'], /^Version:[ \t]+([^\s]+)$/m,
    (line, specVersion) => {
      requireCondition(specVersion === oldVersion, 'RPM version differs from Cargo.toml.')
      return line.replace(oldVersion, version)
    }, 'RPM version')
  const rpmDate = parsedDate.toUTCString().slice(0, 16).replace(',', '').split(' ')
  const rpmEntry = `* ${rpmDate[0]} ${rpmDate[2]} ${rpmDate[1]} ${rpmDate[3]} Browsey Maintainers <maintainers@example.com> - ${version}-1\n- Release ${version}; see CHANGELOG.md and docs/releases/${version}.md\n\n`
  add('packaging/rpm/browsey.spec', replaceOnce(spec, /^%changelog\n/m, `%changelog\n${rpmEntry}`, 'RPM changelog'))
  const appstream = files['packaging/com.browsey.metainfo.xml']
  requireCondition(appstream.match(/<release version="([^"]+)"/)?.[1] === oldVersion,
    'Latest AppStream release differs from Cargo.toml.')
  const previousDate = appstream.match(/<release version="[^"]+" date="([^"]+)"/)?.[1]
  requireCondition(parsedDate >= releaseDate(previousDate), 'Release date predates the current AppStream release.')
  requireCondition(!appstream.includes(`<release version="${version}"`), 'AppStream already contains the requested version.')
  add('packaging/com.browsey.metainfo.xml', replaceOnce(appstream, /  <releases>\n/,
    `  <releases>\n    <release version="${version}" date="${date}"/>\n`, 'AppStream releases'))

  const changelog = files['CHANGELOG.md']
  requireCondition(changelog.match(/^## v([^\s]+) /m)?.[1] === oldVersion, 'Latest changelog release differs from Cargo.toml.')
  requireCondition(!changelog.includes(`## v${version} `), 'Changelog already contains the requested version.')
  let notes
  add('CHANGELOG.md', replaceOnce(changelog, /^## Unreleased\n([\s\S]*?)(?=^## |$(?![\s\S]))/m,
    (_, body) => {
      notes = body.trim()
      requireCondition(/^[-*] /m.test(notes), 'Unreleased must contain at least one change before a bump.')
      return `## Unreleased\n\n## v${version} — ${date}\n\n${notes}\n\n`
    }, 'Unreleased changelog section'))

  let readme = replaceOnce(files['README.md'], /^Downloads: .*$/m,
    (line) => {
      requireCondition(line.includes(`/releases/tag/v${oldVersion})`), 'README download tag differs from Cargo.toml.')
      return replaceVersion(line, oldVersion, version, 'README downloads')
    }, 'README downloads')
  readme = replaceOnce(readme, /^## Status\n([^\n]+)$/m,
    (_, status) => {
      requireCondition(status.startsWith(`Browsey \`${oldVersion}\` `), 'README status version differs from Cargo.toml.')
      return `## Status\n${replaceVersion(status, oldVersion, version, 'README status')
        .replace(/^Browsey `[^`]+` .*?\. /, `Browsey \`${version}\` is Linux-first. `)}`
    }, 'README status')
  readme = replaceOnce(readme, /^- Windows: .*no new Windows installer.*$/m,
    (line) => replaceVersion(line, oldVersion, version, 'README Windows scope'), 'README Windows scope')
  add('README.md', readme)

  let pages = replaceOnce(files['docs-site/src/content/pages.ts'], /        id: 'status',\n        title: 'Current Status',\n        body: '[^\n]*',/,
    (block) => {
      requireCondition(block.includes(`Browsey ${oldVersion} `), 'Docs current status differs from Cargo.toml.')
      return `        id: 'status',\n        title: 'Current Status',\n        body: 'Browsey ${version} is Linux-first. Windows support is in maintenance mode; ${version} packages target Linux x86_64. See release notes for changes and validation scope.',`
    }, 'Docs current status')
  pages = replaceOnce(pages, /^.*Windows NSIS can be built from source; no new Windows installer.*$/m,
    (line) => replaceVersion(line, oldVersion, version, 'Docs Windows scope'), 'Docs Windows scope')
  requireCondition(pages.match(/title: 'v(\d+\.\d+\.\d+) \(/)?.[1] === oldVersion,
    'Latest docs release differs from Cargo.toml.')
  requireCondition(!pages.includes(`title: 'v${version} (`), 'Docs already contain the requested release.')
  // Keep Markdown source in the release document. Flatten list continuations for
  // the site's plain-text bullets and let JSON escaping handle quotes/backslashes.
  const bullets = [...notes.matchAll(/^[-*] (.*(?:\n[ \t]+[^\n]+)*)/gm)]
    .map((match) => match[1].replace(/\n[ \t]+/g, ' '))
  const card = `      {\n        id: 'v${version.replaceAll('.', '-')}',\n        title: 'v${version} (${date})',\n        bullets: [\n${bullets.map((bullet) => `          ${JSON.stringify(bullet)},`).join('\n')}\n        ],\n        note: 'See CHANGELOG.md and docs/releases/${version}.md for full changes. Release preparation does not confirm testing or publication; review validation evidence before publishing.',\n      },\n`
  add('docs-site/src/content/pages.ts', replaceOnce(pages, /      \{\n        id: 'unreleased',\n        title: 'Unreleased',\n        bullets: \[\],\n      \},\n/,
    (block) => `${block}${card}`, 'Docs Unreleased section (must be empty; curate separately if populated)'))
  add(`docs/releases/${version}.md`, `# Browsey ${version}\n\nPlanned release date: ${date}. Target: Linux x86_64 (RPM and DEB).\n\n> Release preparation only: this document is not proof of testing or publication.\n> Review the changes, platform scope and development/unreleased labels throughout\n> README and the docs site before tagging. Fill in validation evidence and remove\n> this notice only when the release is ready.\n\n## Changes\n\n${notes}\n\n## Validation and publication checklist\n\n- [ ] Run \`bash scripts/maintenance/test-both.sh --strict-docs\`.\n- [ ] Run docs lint, typecheck and production build.\n- [ ] Review security audits and CI results; record any skipped checks.\n- [ ] Record fresh manual checks for affected workflows, OS/session and tested commit.\n- [ ] Commit/push main, then create and push annotated tag \`v${version}\` on the tested commit.\n- [ ] Dispatch Linux Release Bundles with that tag; inspect RPM/DEB versions, dependencies and startup results.\n- [ ] Download packages, verify SHA256SUMS and validate installation/upgrade on claimed target systems.\n- [ ] Review limitations, create a draft GitHub release, upload RPM/DEB/SHA256SUMS and publish.\n- [ ] Back up the local installation; finish active file operations before restarting and verify the installed version.\n\nValidation evidence, workflow links, package checksums and known limitations must\nbe recorded here before publication. Historical checklists are not fresh signoff.\nDo not overwrite older release notes, tags or release assets. No Windows installer\nis produced by the Linux release workflow; revise scope if publishing one separately.\n`)
  return { oldVersion, version, date, changes }
}

function git(root, args, input) {
  const result = spawnSync('git', args, { cwd: root, input, encoding: 'utf8', maxBuffer: 10 * 1024 * 1024, timeout: 30_000 })
  requireCondition(!result.error && result.status === 0,
    `git ${args[0]} failed: ${result.error?.message ?? result.stderr.trim()}`)
  return result.stdout
}

function patchFor(plan) {
  return plan.changes.map(({ path, before, after }) => {
    requireCondition(after.endsWith('\n') && (before === null || before.endsWith('\n')),
      `${path}: expected newline-terminated files.`)
    const oldLines = before === null ? [] : before.slice(0, -1).split('\n')
    const newLines = after.slice(0, -1).split('\n')
    return `diff --git a/${path} b/${path}\n${before === null ? 'new file mode 100644\n' : ''}`
      + `--- ${before === null ? '/dev/null' : `a/${path}`}\n+++ b/${path}\n`
      + `@@ -${before === null ? 0 : 1},${oldLines.length} +1,${newLines.length} @@\n`
      + oldLines.map((line) => `-${line}\n`).join('') + newLines.map((line) => `+${line}\n`).join('')
  }).join('')
}

function checkTags(root, version) {
  const tag = `v${version}`
  requireCondition(!git(root, ['tag', '--list', tag]).trim(), `Local tag ${tag} already exists.`)
  if (git(root, ['remote']).split('\n').includes('origin')) {
    requireCondition(!git(root, ['ls-remote', '--tags', 'origin', `refs/tags/${tag}`, `refs/tags/${tag}^{}`]).trim(),
      `Origin tag ${tag} already exists.`)
  }
}

function requireClean(root) {
  requireCondition(!git(root, ['status', '--porcelain', '--untracked-files=normal']).trim(),
    'Working tree is not clean. Commit or preserve pending changes before --apply; preview is still available.')
}

export function applyPlan(root, plan) {
  requireClean(root)
  checkTags(root, plan.version)
  for (const { path, before } of plan.changes) {
    const fullPath = join(root, path)
    requireCondition(before === null ? !existsSync(fullPath) : readFileSync(fullPath, 'utf8') === before,
      `${path} changed after planning; no bump applied.`)
  }
  const patch = patchFor(plan)
  git(root, ['apply', '--check', '--whitespace=error'], patch)
  // Git preflights the complete patch; never use --reject or --3way here.
  git(root, ['apply', '--whitespace=error'], patch)
}

export function parseArgs(args) {
  const options = { apply: false, verify: false, date: new Date().toISOString().slice(0, 10) }
  for (let index = 0; index < args.length; index++) {
    const arg = args[index]
    if (arg === '--apply' || arg === '--verify') {
      const key = arg.slice(2)
      requireCondition(!options[key], `Duplicate option: ${arg}`)
      options[key] = true
    } else if (arg === '--date') {
      requireCondition(!options.explicitDate && args[index + 1], '--date requires one YYYY-MM-DD value.')
      options.date = args[++index]
      options.explicitDate = true
    } else if (!arg.startsWith('-') && !options.version) {
      options.version = arg
    } else {
      throw new Error(`Unknown or duplicate argument: ${arg}`)
    }
  }
  versionParts(options.version)
  releaseDate(options.date)
  requireCondition(!options.verify || options.apply, '--verify requires --apply; previews do not run builds or tests.')
  return options
}

export function main(args, root = repoRoot) {
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) {
    console.log('Usage: node scripts/release/bump.mjs X.Y.Z [--date YYYY-MM-DD] [--apply [--verify]]\n'
      + 'Default: read-only preview. --apply requires a clean working tree.\n'
      + '--verify runs strict maintenance and docs suites AFTER applying the bump.\n'
      + 'No commit, push, tag, publication, installation or dependency update is performed.')
    return
  }
  const options = parseArgs(args)
  requireCondition(resolve(git(root, ['rev-parse', '--show-toplevel']).trim()) === resolve(root),
    'Run the helper from a Browsey repository checkout.')
  if (options.apply) requireClean(root)
  requireCondition(!Object.hasOwn(JSON.parse(readFileSync(join(root, 'tauri.conf.json'), 'utf8')), 'version'),
    'Tauri has an explicit version; update the helper before bumping.')
  const files = Object.fromEntries(versionFiles.map((path) => [path, readFileSync(join(root, path), 'utf8')]))
  const plan = buildPlan(files, options.version, options.date)
  requireCondition(!existsSync(join(root, `docs/releases/${options.version}.md`)), 'Release notes already exist; refusing to overwrite.')
  checkTags(root, options.version)
  console.log(`${options.apply ? 'Preparing' : 'Preview only:'} Browsey ${plan.oldVersion} → ${plan.version} (${plan.date})`)
  for (const change of plan.changes) console.log(`  ${change.before === null ? 'create' : 'update'} ${change.path}`)
  if (!options.apply) {
    console.log('No files changed. Use --apply after committing pending work.\nOrigin tags are checked when origin is configured; GitHub releases/assets are reviewed manually.')
    return
  }
  applyPlan(root, plan)
  console.log('Bump applied. Review git diff, release notes and remaining development/unreleased labels. Nothing published.')
  if (options.verify) {
    const commands = [
      ['bash', ['scripts/maintenance/test-both.sh', '--strict-docs']],
      ...['lint', 'check', 'build'].map((task) => ['npm', ['--prefix', 'docs-site', 'run', task]]),
    ]
    for (const [command, commandArgs] of commands) {
      const result = spawnSync(command, commandArgs, { cwd: root, stdio: 'inherit' })
      requireCondition(!result.error && result.status === 0,
        `${command} ${commandArgs.join(' ')} failed. Bump remains for inspection; not tagged or published.`)
    }
  }
  console.log('Next: review CI/security/manual evidence, commit/push, tag the tested commit, dispatch Linux Release Bundles, then review and publish a draft release.')
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`Bump aborted: ${error.message}`)
    process.exitCode = 1
  }
}
