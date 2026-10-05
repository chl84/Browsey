import assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import { regularFile } from './fixtures.mjs'
import { validateConfig, child, noLinks } from './scope.mjs'

// Explicit one-time credential exception, not automatic discovery/import.
// The maintainer must approve reading this exact existing source config first.
const [permission, source, local, remote] = process.argv.slice(2)
assert.equal(permission, '--approved-existing-config', 'Explicit approval of the existing config is required')
const destination = child(local, 'rclone.conf')
validateConfig({ schema: 1, targets: { local, cloud: `rclone://${remote}/ai_agent_testfolder` }, rcloneConfig: destination })
await noLinks(local, fs)
assert.ok((await fs.lstat(local)).isDirectory(), 'Never create a missing approved test folder')
const original = await regularFile(source)
const sections = original.text.split(/(?=^\[[^\]\r\n]+\]\s*$)/m)
const selected = sections.filter(section => section.match(/^\[([^\]\r\n]+)\]/)?.[1] === remote)
assert.equal(selected.length, 1, 'Exactly one approved remote section is required')
// An alias/crypt/combine could refer to other accounts or paths outside approval.
assert.match(selected[0], /^type\s*=\s*(onedrive|drive|webdav)\s*$/m, 'Expected a directly configured supported provider')
await noLinks(destination, fs)
await fs.writeFile(destination, selected[0], { flag: 'wx', mode: 0o600 })
console.log('Created a mode-600 test-only credential copy for the approved remote. Original unchanged; no credentials printed.')
