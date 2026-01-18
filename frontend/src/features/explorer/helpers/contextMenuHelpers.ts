export const ensureSelectionBeforeMenu = (
  currentSelection: Set<string>,
  path: string,
  index: number,
  onSelect: (paths: Set<string>, anchor: number | null, caret: number | null) => void,
) => {
  if (currentSelection.has(path)) return
  const next = new Set<string>([path])
  onSelect(next, index, index)
}
