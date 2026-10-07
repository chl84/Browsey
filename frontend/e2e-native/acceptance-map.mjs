import assert from 'node:assert/strict'
// Links express bounded supporting evidence, never broad-row signoff.
export const acceptanceRules = Object.freeze([
  {family: 'cloud web restore', pattern: /^provider-cloud-web-restore$/, rows: ['A0-2', 'A4-3']},
  {family: 'cloud export', pattern: /^desktop-cloud-export$/, rows: ['A1-1', 'A4-1']},
  {family: 'cloud mixed-size scale', pattern: /^provider-cloud-scale$/, rows: ['A0-2', 'A4-4']},
  {family: 'storage performance', pattern: /^storage-performance-(local|usb|mobile|cloud)$/, rows: ['A2-3', 'A4-4']},
  {family: 'desktop-drag', pattern: /^desktop-drag$/, rows: ['A1-1', 'A2-1', 'A2-6']},
  {family: 'desktop-feedback', pattern: /^desktop-feedback$/, rows: ['A1-1', 'A2-3', 'A2-5']},
  {family: 'desktop-services', pattern: /^desktop-services$/, rows: ['A0-2', 'A1-2']},
  {family: 'desktop-keyboard', pattern: /^desktop-keyboard$/, rows: ['A2-1', 'A2-2', 'A2-7', 'A3-1']},
  {family: 'desktop-appearance', pattern: /^desktop-appearance$/, rows: ['A2-3', 'A2-4', 'A3-2']},
  {family: 'desktop-watchers', pattern: /^desktop-watchers$/, rows: ['A0-7', 'A2-1', 'A2-6']},
  {family: 'desktop-archives', pattern: /^desktop-archives-(local|cloud)$/, rows: ['A0-2', 'A0-3', 'A2-2', 'A2-5']},
  {family: 'desktop-open-with', pattern: /^desktop-open-with$/, rows: ['A1-3', 'A2-2']},
  {family: 'accessibility', pattern: /^accessibility-local$/, rows: ['A3-1']},
  {family: 'owned lifecycle', pattern: /^(lifecycle-(owned-window|profile-reuse)|interruption-local)$/, rows: ['A0-4', 'A2-6']},
  {family: 'undo/history', pattern: /^(undo-copy-local|history-local)(-(fresh|reused))?$/, rows: ['A0-5']},
  {family: 'active writes', pattern: /^races-/, rows: ['A0-7', 'A0-3']},
  {family: 'cancel/failure/overwrite', pattern: /^(cancel|moves|iofaults|overwrite)-/, rows: ['A0-2', 'A0-3', 'A2-4', 'A2-5']},
  {family: 'partial batches', pattern: /^batch-/, rows: ['A0-2', 'A0-3', 'A2-5']},
  {family: 'byte progress', pattern: /^progress-/, rows: ['A2-4', 'A2-5']},
  {family: 'OneDrive', pattern: /^provider-cloud(-errors)?$/, rows: ['A0-2', 'A4-1', 'A4-2', 'A4-3', 'A4-4']},
  {family: 'MTP', pattern: /^provider-mobile$/, rows: ['A1-5', 'A4-1', 'A4-4']},
  {family: 'network', pattern: /^provider-network$/, rows: ['A0-2', 'A1-7']},
  {family: 'navigation/listing', pattern: /^(navigation|listing)-/, rows: ['A2-1', 'A4-4']},
  {family: 'selection', pattern: /^selection-/, rows: ['A2-1']},
  {family: 'Properties', pattern: /^properties-/, rows: ['A2-2', 'A4-1']},
  {family: 'Hidden rename/delete', pattern: /^hidden-/, rows: ['A0-2', 'A2-2', 'A4-1']},
  {family: 'input/create/rename', pattern: /^(input|create|creation|rename)-/, rows: ['A0-2', 'A2-1', 'A2-2']},
  {family: 'copy/move/fileops', pattern: /^(copy|move|fileops)-/, rows: ['A0-2', 'A0-3']},
  {family: 'delete', pattern: /^delete-/, rows: ['A0-2', 'A0-3', 'A4-3']},
  {family: 'access/guards/links', pattern: /^(access|guards|links)-/, rows: ['A0-2', 'A0-3', 'A4-1']},
  {family: 'names/limits/contents/trees', pattern: /^(names|limits|contents|trees)-/, rows: ['A0-2', 'A2-1', 'A2-3']},
])
export function acceptanceRows(id) {
  assert.equal(typeof id, 'string')
  const rule = acceptanceRules.find(rule => rule.pattern.test(id))
  assert.ok(rule, 'Native case has no explicit acceptance mapping')
  return [...rule.rows]
}
