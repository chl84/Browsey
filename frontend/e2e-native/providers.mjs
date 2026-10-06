import assert from 'node:assert/strict'
import {foundation,foundationManifest} from './cases.mjs'
import {access,accessManifest} from './access.mjs'

export function usbManifest(plan) {
  assert.deepEqual(plan.targets.map(t=>t.kind),['local','usb'],'USB acceptance requires only local and USB')
  return [...foundationManifest(plan),...accessManifest(plan,'usb')]
}
export async function usb(plan,fixture,ui,record) {
  usbManifest(plan)
  ui.waitTimeout=180_000;ui.transferTimeout=600_000
  await foundation(plan,fixture,ui,record)
  await access(plan,fixture,ui,record,'usb')
}
