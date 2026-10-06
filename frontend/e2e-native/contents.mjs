import assert from 'node:assert/strict'
import { child } from './scope.mjs'
import { progressRoutes, waitActivityGone } from './progress.mjs'
import { transferListeners } from './cancellation.mjs'
import { verifyByteTree } from './byte-tree.mjs'
import { recordPart } from './report.mjs'

const idFor = route => `contents-${route.from}-${route.to}`
export function generatedBinary(size, salt = 0) {
  assert.ok(Number.isSafeInteger(size) && size >= 0 && size <= 65536)
  return Buffer.from(Array.from({ length: size }, (_, index) => (index * 197 + salt * 31) & 255))
}
export const contentsManifest = plan => progressRoutes(plan).map(route => ({ id: idFor(route),
  name: `Binary readback: ${route.from} to ${route.to}`, providers: [...new Set([route.from, route.to])], partIds: ['copy', 'move'] }))
export async function contents(plan, fixture, ui, record) {
  ui.waitTimeout = 180_000; ui.transferTimeout = 600_000
  for (const [salt, route] of progressRoutes(plan).entries()) await record(`Binary readback: ${route.from} to ${route.to}`, async result => {
    result.phase = 'setup'
    const from = child(plan.targets.find(target => target.kind === route.from).files, `${idFor(route)}-source`)
    const copied = child(plan.targets.find(target => target.kind === route.to).files, `${idFor(route)}-copied`)
    const moved = child(plan.targets.find(target => target.kind === route.to).files, `${idFor(route)}-moved`)
    const data = new Map([['zero.bin', Buffer.alloc(0)], ['one.bin', Buffer.from([255])],
      ['irregular.bin', generatedBinary(4097, salt)], ['full.bin', generatedBinary(65536, salt)]])
    const source = new Map([...data, ['source-sentinel.txt', Buffer.from(`Preserve unrelated source ${salt}.\n`)]])
    const copyExpected = new Map([['destination-sentinel.txt', Buffer.from(`Preserve unrelated copy target ${salt}.\n`)]])
    const moveExpected = new Map([['destination-sentinel.txt', Buffer.from(`Preserve unrelated move target ${salt}.\n`)]])
    for (const [base, expected] of [[from, source], [copied, copyExpected], [moved, moveExpected]]) {
      await fixture.mkdir(base)
      for (const [name, bytes] of expected) await fixture.write(child(base, name), bytes)
    }
    for (const move of [false, true]) await recordPart(result, move ? 'move' : 'copy', async part => {
      result.phase = 'ui'; part.ui = 'STARTED'; await ui.setView(move ? 'grid' : 'list')
      await ui.populateClipboard(from, [...data.keys()].map(name => child(from, name)), move, move)
      const target = move ? moved : copied, targetExpected = move ? moveExpected : copyExpected
      await ui.paste(target, child(target, 'zero.bin'), { menu: move })
      await waitActivityGone(ui)
      part.ui = 'ACKNOWLEDGED'; result.phase = 'verification'
      for (const [name, bytes] of data) { targetExpected.set(name, bytes); if (move) source.delete(name) }
      part.source = await verifyByteTree(fixture, from, source)
      part.destination = await verifyByteTree(fixture, target, targetExpected)
      if (move) await verifyByteTree(fixture, copied, copyExpected)
      assert.equal((await ui.handshake(plan.runId)).cancelTasks, 0)
      assert.deepEqual(await transferListeners(ui), [])
    })
  }, { id: idFor(route), providers: [...new Set([route.from, route.to])] })
}
