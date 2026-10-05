import assert from 'node:assert/strict'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { validateConfig } from './scope.mjs'
import { privateJson } from './privacy.mjs'
import { RetentionStore } from './retention.mjs'

process.umask(0o077)
const repo = fileURLToPath(new URL('../..', import.meta.url))
const [mode = '--help', runId, planSha256, ...extra] = process.argv.slice(2)

async function main() {
  if (mode === '--help') {
    console.log('Native retention: --audit UUID | --review UUID | --plan UUID | --cleanup UUID PLAN_SHA256\nNo root scans, automatic expiry deletion, legacy/failed/active/multi-provider cleanup or force flag.')
    return
  }
  assert.ok(['--audit', '--review', '--plan', '--cleanup'].includes(mode) && !extra.length)
  assert.equal(Boolean(planSha256), mode === '--cleanup', 'Only cleanup accepts the reviewed plan SHA-256')
  const config = validateConfig(await privateJson(path.join(repo, 'frontend/e2e-native/config.local.json')))
  const store = new RetentionStore(path.join(repo, 'target/native-test/.retention'))
  const result = mode === '--audit' ? await store.audit(config, runId)
    : mode === '--review' ? await store.review(config, runId)
      : mode === '--plan' ? await store.plan(config, runId) : await store.cleanup(config, runId, planSha256)
  console.log(JSON.stringify(result, null, 2))
}

main().catch(error => { console.error(`Native retention stopped: ${error.message}; recovery data retained, no automatic retry`); process.exitCode = 1 })
