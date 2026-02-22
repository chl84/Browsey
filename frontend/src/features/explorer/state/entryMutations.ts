import type { Entry } from '../model/types'

export const removeEntryByPath = (list: Entry[], path: string): Entry[] =>
  list.filter((entry) => entry.path !== path)

export const patchEntryStarred = (list: Entry[], path: string, starred: boolean): Entry[] =>
  list.map((entry) => (entry.path === path ? { ...entry, starred } : entry))

export const appendEntries = (list: Entry[], appended: Entry[]): Entry[] =>
  appended.length === 0 ? list : [...list, ...appended]

