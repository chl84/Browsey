import type { Entry } from '../model/types'

// Linux remote trash is decided by the backend, not the listing's network flag.
// Windows keeps its existing permanent-delete flow for network locations.
export const mustUsePermanentDelete = (
  entries: Entry[], permanent: boolean, inTrashView: boolean, windows: boolean,
) => {
  const hasCloud = entries.some(entry => entry.path.startsWith('rclone://'))
  const hasNetwork = entries.some(entry => entry.network)
  const cloudTrash = hasCloud && entries.every(entry => entry.capabilities?.canTrash)
  return permanent || (!inTrashView && ((windows && hasNetwork && !hasCloud) || (hasCloud && !cloudTrash)))
}
