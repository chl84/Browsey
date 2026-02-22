import type { Entry } from '../model/types'

type FormatSelectionLine = (count: number, singular: string, bytes?: number) => string

export const buildExplorerSelectionText = ({
  entries,
  selectedPaths,
  filterValue,
  filteredCount,
  formatSelectionLine,
}: {
  entries: Entry[]
  selectedPaths: Set<string>
  filterValue: string
  filteredCount: number
  formatSelectionLine: FormatSelectionLine
}) => {
  const selectedEntries = entries.filter((e) => selectedPaths.has(e.path))
  const files = selectedEntries.filter((e) => e.kind === 'file')
  const links = selectedEntries.filter((e) => e.kind === 'link')
  const dirs = selectedEntries.filter((e) => e.kind === 'dir')
  const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
  const fileCount = files.length + links.length

  const dirLine = formatSelectionLine(dirs.length, 'folder')
  const fileLine = formatSelectionLine(fileCount, 'file', fileBytes)
  const hasFilter = filterValue.trim().length > 0
  const filterLine = hasFilter ? `${filteredCount} results` : ''

  return [filterLine, dirLine, fileLine].filter((p) => p.length > 0).join(' | ')
}

export const computeSelectionAnchorRepair = ({
  list,
  selectedPaths,
  anchorIndex,
  caretIndex,
}: {
  list: Entry[]
  selectedPaths: Set<string>
  anchorIndex: number | null
  caretIndex: number | null
}) => {
  if (selectedPaths.size === 0 || list.length === 0) {
    return null
  }

  const firstIdx = list.findIndex((e) => selectedPaths.has(e.path))
  if (firstIdx < 0) {
    return null
  }

  const anchorValid =
    anchorIndex !== null &&
    anchorIndex < list.length &&
    selectedPaths.has(list[anchorIndex].path)
  const caretValid =
    caretIndex !== null &&
    caretIndex < list.length &&
    selectedPaths.has(list[caretIndex].path)

  return {
    nextAnchorIndex: anchorValid ? null : firstIdx,
    nextCaretIndex: caretValid ? null : firstIdx,
  }
}
