import { promises as fs } from 'node:fs'
import path from 'node:path'

const ROOT = path.resolve('src')
const ALLOWED_USE_CREATE_FILES = new Set([
  'src/features/settings/hooks/useSettingsModalViewModel.ts',
])

const USE_FILE_RE = /\/use[^/]+\.(ts|js)$/
const CREATE_EXPORT_RE = /\bexport\s+(?:const|function)\s+create[A-Z]\w*\b/g
const USE_EXPORT_RE = /\bexport\s+(?:const|function)\s+use[A-Z]\w*\b/g

const walk = async (dir) => {
  const entries = await fs.readdir(dir, { withFileTypes: true })
  const files = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(dir, entry.name)
      if (entry.isDirectory()) return walk(fullPath)
      if (entry.isFile()) return [fullPath]
      return []
    })
  )
  return files.flat()
}

const toPosixRel = (fullPath) =>
  path.relative(process.cwd(), fullPath).split(path.sep).join(path.posix.sep)

const main = async () => {
  const allFiles = await walk(ROOT)
  const useFiles = allFiles.filter((fullPath) => USE_FILE_RE.test(toPosixRel(fullPath)))
  const violations = []

  for (const filePath of useFiles) {
    const relPath = toPosixRel(filePath)
    if (ALLOWED_USE_CREATE_FILES.has(relPath)) continue

    const src = await fs.readFile(filePath, 'utf8')
    const hasCreateExport = CREATE_EXPORT_RE.test(src)
    const hasUseExport = USE_EXPORT_RE.test(src)
    CREATE_EXPORT_RE.lastIndex = 0
    USE_EXPORT_RE.lastIndex = 0

    if (hasCreateExport && !hasUseExport) {
      violations.push(relPath)
    }
  }

  if (violations.length === 0) {
    console.log('Naming conventions check passed.')
    return
  }

  console.error('Naming conventions check failed.')
  console.error('The following use*.ts/js files export create* without a use* export:')
  for (const file of violations) {
    console.error(`- ${file}`)
  }
  console.error('Rename the file to create*.ts/js or export a use* symbol.')
  process.exit(1)
}

await main()
