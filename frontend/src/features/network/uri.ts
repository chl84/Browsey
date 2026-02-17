import type { Partition } from '../explorer/types'

export const uriScheme = (value: string): string | null => {
  const trimmed = value.trim()
  const idx = trimmed.indexOf('://')
  if (idx <= 0) return null
  const raw = trimmed.slice(0, idx).toLowerCase()
  return raw === 'ssh' ? 'sftp' : raw
}

export const isMountUri = (value: string): boolean => uriScheme(value) !== null

const uriHost = (value: string): string | null => {
  const idx = value.indexOf('://')
  if (idx <= 0) return null
  const remainder = value.slice(idx + 3)
  if (!remainder) return null
  const authority = remainder.split('/')[0]?.trim() ?? ''
  if (!authority) return null
  const withoutUser = authority.includes('@') ? authority.split('@').pop() ?? authority : authority
  if (!withoutUser) return null
  if (withoutUser.startsWith('[')) {
    const end = withoutUser.indexOf(']')
    if (end > 1) {
      return withoutUser.slice(1, end).toLowerCase()
    }
  }
  return (withoutUser.split(':')[0] ?? '').trim().toLowerCase() || null
}

export const resolveMountedPathForUri = (
  uri: string,
  mounts: Partition[],
): string | null => {
  const scheme = uriScheme(uri)
  if (!scheme) return null

  const mounted = mounts.filter((mount) => {
    const fsLc = (mount.fs ?? '').toLowerCase()
    return fsLc === scheme && !isMountUri(mount.path)
  })
  if (mounted.length === 0) return null
  if (scheme !== 'sftp') {
    return mounted[0].path
  }

  const host = uriHost(uri)
  if (!host) return mounted[0].path
  const byHost = mounted.find((mount) => mount.path.toLowerCase().includes(`host=${host}`))
  return byHost?.path ?? mounted[0].path
}
