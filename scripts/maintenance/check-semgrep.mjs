import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export function scanArgs(mode) {
  if (mode !== 'advisory' && mode !== 'blocking') {
    throw new Error('Expected advisory or blocking mode')
  }
  return [
    'scan',
    '--metrics=off',
    '--disable-version-check',
    '--strict',
    ...(mode === 'blocking' ? ['--error'] : []),
    '--config',
    mode === 'blocking' ? '.semgrep/typed-errors-blocking.yml' : '.semgrep/typed-errors.yml',
    mode === 'blocking' ? 'src/commands' : 'src',
  ]
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [mode, format, ...extra] = process.argv.slice(2)
  try {
    if (extra.length || (format !== undefined && format !== '--json')) {
      throw new Error('Usage: node scripts/maintenance/check-semgrep.mjs advisory|blocking [--json]')
    }
    const result = spawnSync('semgrep', [...scanArgs(mode), ...(format ? [format] : [])], {
      stdio: 'inherit',
    })
    if (result.error) console.error(result.error.message)
    process.exitCode = result.status ?? 2
  } catch (error) {
    console.error(error.message)
    process.exitCode = 2
  }
}
