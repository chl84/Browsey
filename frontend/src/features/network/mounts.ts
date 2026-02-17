import type { Entry, Partition } from '../explorer/types'
import { normalizePath } from '../explorer/utils'

const NETWORK_FS = new Set([
  'mtp',
  'onedrive',
  'cifs',
  'smb3',
  'smbfs',
  'nfs',
  'nfs4',
  'sshfs',
  'fuse.sshfs',
  'davfs2',
  'afpfs',
  'ftpfs',
  'curlftpfs',
])

export const isNetworkMount = (mount: Partition): boolean => {
  const path = mount.path.trim()
  if (!path) return false
  const pathLc = path.toLowerCase()
  const fsLc = (mount.fs ?? '').toLowerCase()

  if (pathLc.startsWith('onedrive://')) return true
  if (pathLc.includes('/gvfs/') || pathLc.includes('\\gvfs\\')) return true
  if (NETWORK_FS.has(fsLc)) return true
  return false
}

export const toNetworkEntries = (mounts: Partition[]): Entry[] => {
  const onedriveMounted = mounts.some((mount) => {
    const fsLc = (mount.fs ?? '').toLowerCase()
    const pathLc = mount.path.trim().toLowerCase()
    return fsLc === 'onedrive' && !pathLc.startsWith('onedrive://')
  })

  const deduped = new Map<string, Partition>()
  for (const mount of mounts) {
    if (!isNetworkMount(mount)) continue
    const rawPath = mount.path.trim()
    const rawPathLc = rawPath.toLowerCase()
    if (onedriveMounted && rawPathLc.startsWith('onedrive://')) {
      continue
    }
    const normalized = normalizePath(rawPath)
    const key = normalized || rawPath
    if (!key) continue
    if (rawPath.includes('://') && !rawPathLc.startsWith('onedrive://')) {
      continue
    }
    if (!deduped.has(key)) {
      deduped.set(key, mount)
    }
  }

  return Array.from(deduped.values()).map((mount) => ({
    name: mount.label?.trim() || mount.path,
    path: mount.path,
    kind: 'dir',
    iconId: 10,
    network: true,
  }))
}
