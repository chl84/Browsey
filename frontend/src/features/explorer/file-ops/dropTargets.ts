import type { DropPosition } from './createNativeFileDrop'

export type DropTarget = { path: string; element: HTMLElement; openOnHover: boolean }

export const isDropDirectoryPath = (path: string) =>
  path.startsWith('/') || /^[a-z]:[\\/]/i.test(path) || path.startsWith('\\\\') || /^rclone:\/\/[^/]+/.test(path)

export const findDropTarget = (point: DropPosition, backgroundPath: string | null): DropTarget | null => {
  const hit = document.elementFromPoint(point.x, point.y)
  if (!hit || hit.closest('[role="dialog"], [data-drop-blocked]')) return null
  // A file, disabled destination, or control is an explicit rejection, never a background drop.
  const element = hit.closest<HTMLElement>('[data-drop-path], [data-drop-background]')
  if (!element) return null
  const background = element.hasAttribute('data-drop-background')
  const path = background ? backgroundPath : element.dataset.dropPath
  if (!path || !isDropDirectoryPath(path)) return null
  return { path, element, openOnHover: !background }
}
