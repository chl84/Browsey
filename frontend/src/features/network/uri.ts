import type { Partition } from '../explorer/types'

const canonicalNetworkScheme = (raw: string): string | null => {
  const value = raw.trim().toLowerCase()
  if (!value) return null
  switch (value) {
    case 'onedrive':
      return 'onedrive'
    case 'sftp':
    case 'ssh':
    case 'sshfs':
    case 'fuse.sshfs':
      return 'sftp'
    case 'smb':
    case 'smb3':
    case 'smbfs':
    case 'cifs':
    case 'smb-share':
      return 'smb'
    case 'nfs':
    case 'nfs4':
      return 'nfs'
    case 'ftp':
    case 'ftpfs':
    case 'curlftpfs':
      return 'ftp'
    case 'dav':
    case 'webdav':
    case 'davfs2':
      return 'dav'
    case 'davs':
    case 'webdavs':
      return 'davs'
    case 'afp':
    case 'afpfs':
    case 'afp-volume':
      return 'afp'
    case 'http':
      return 'http'
    case 'https':
      return 'https'
    case 'mtp':
      return 'mtp'
    default:
      return null
  }
}

export const uriScheme = (value: string): string | null => {
  const trimmed = value.trim()
  const idx = trimmed.indexOf('://')
  if (idx <= 0) return null
  const raw = trimmed.slice(0, idx).toLowerCase()
  return canonicalNetworkScheme(raw) ?? raw
}

export const isMountUri = (value: string): boolean => uriScheme(value) !== null

const MOUNTABLE_URI_SCHEMES = new Set([
  'onedrive',
  'sftp',
  'smb',
  'nfs',
  'ftp',
  'dav',
  'davs',
  'afp',
])
const EXTERNAL_URI_SCHEMES = new Set(['http', 'https'])

export const isKnownNetworkUriScheme = (scheme: string | null): boolean =>
  !!scheme && (MOUNTABLE_URI_SCHEMES.has(scheme) || EXTERNAL_URI_SCHEMES.has(scheme))

export const isMountableUri = (value: string): boolean => {
  const scheme = uriScheme(value)
  return !!scheme && MOUNTABLE_URI_SCHEMES.has(scheme)
}

export const isExternallyOpenableUri = (value: string): boolean => {
  const scheme = uriScheme(value)
  return !!scheme && EXTERNAL_URI_SCHEMES.has(scheme)
}

const safeDecode = (value: string): string => {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

const normalizeHost = (value: string): string => {
  const trimmed = value.trim().replace(/\.+$/, '')
  if (!trimmed) return ''
  if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
    return trimmed.slice(1, -1).toLowerCase()
  }
  if (!trimmed.startsWith('[') && trimmed.includes(':') && trimmed.split(':').length === 2) {
    return trimmed.split(':')[0].trim().toLowerCase()
  }
  return trimmed.toLowerCase()
}

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
      return normalizeHost(withoutUser.slice(1, end))
    }
  }
  const host = normalizeHost(withoutUser)
  return host || null
}

const uriPath = (value: string): string => {
  const idx = value.indexOf('://')
  if (idx <= 0) return ''
  const remainder = value.slice(idx + 3)
  if (!remainder) return ''
  const slash = remainder.indexOf('/')
  if (slash < 0) return ''
  return remainder.slice(slash + 1)
}

const normalizePathForCompare = (value: string): string => {
  const decoded = safeDecode(value).trim()
  if (!decoded) return '/'
  const withLeadingSlash = decoded.startsWith('/') ? decoded : `/${decoded}`
  const collapsed = withLeadingSlash.replace(/\/+$/, '')
  return (collapsed || '/').toLowerCase()
}

const normalizeSegment = (value: string): string =>
  safeDecode(value)
    .trim()
    .replace(/^\/+/, '')
    .replace(/\/+$/, '')
    .toLowerCase()

const gvfsEntryName = (path: string): string | null => {
  const normalizedPath = path.replace(/\\/g, '/')
  const marker = '/gvfs/'
  const idx = normalizedPath.toLowerCase().indexOf(marker)
  if (idx < 0) return null
  const entry = normalizedPath.slice(idx + marker.length).split('/')[0]?.trim() ?? ''
  return entry || null
}

const gvfsParams = (path: string): Map<string, string> => {
  const entry = gvfsEntryName(path)
  if (!entry) return new Map()
  const argsIdx = entry.indexOf(':')
  if (argsIdx < 0 || argsIdx >= entry.length - 1) return new Map()
  const args = entry.slice(argsIdx + 1)
  const out = new Map<string, string>()
  for (const token of args.split(',')) {
    const [rawKey, rawValue] = token.split('=', 2)
    if (!rawKey || rawValue === undefined) continue
    const key = rawKey.trim().toLowerCase()
    if (!key) continue
    out.set(key, safeDecode(rawValue.trim()))
  }
  return out
}

const mountScheme = (mount: Partition): string | null => {
  const fromFs = canonicalNetworkScheme(mount.fs ?? '')
  if (fromFs) return fromFs
  const entry = gvfsEntryName(mount.path)
  if (!entry) return null
  const prefix = entry.split(':', 1)[0] ?? ''
  return canonicalNetworkScheme(prefix)
}

type MountedCandidate = {
  mount: Partition
  params: Map<string, string>
  host: string | null
}

const uriFirstSegment = (value: string): string | null => {
  const path = uriPath(value)
  if (!path) return null
  const first = path.split('/').find((segment) => segment.length > 0)
  if (!first) return null
  const normalized = normalizeSegment(first)
  return normalized || null
}

export const resolveMountedPathForUri = (
  uri: string,
  mounts: Partition[],
): string | null => {
  const scheme = uriScheme(uri)
  if (!scheme) return null

  const mounted: MountedCandidate[] = mounts
    .map((mount) => {
      if (isMountUri(mount.path)) return null
      const fsScheme = mountScheme(mount)
      if (fsScheme !== scheme) return null
      const params = gvfsParams(mount.path)
      const host = normalizeHost(params.get('host') ?? params.get('server') ?? '')
      return {
        mount,
        params,
        host: host || null,
      }
    })
    .filter((item): item is MountedCandidate => item !== null)
  if (mounted.length === 0) return null

  let candidates = mounted
  const host = uriHost(uri)
  if (host) {
    const strictHostMatches = candidates.filter((item) => item.host === host)
    if (strictHostMatches.length > 0) {
      candidates = strictHostMatches
    }
  }

  const firstSegment = uriFirstSegment(uri)
  if (firstSegment && (scheme === 'smb' || scheme === 'afp')) {
    const key = scheme === 'smb' ? 'share' : 'volume'
    const bySegment = candidates.filter((item) => normalizeSegment(item.params.get(key) ?? '') === firstSegment)
    if (bySegment.length > 0) {
      candidates = bySegment
    }
  }

  if (scheme === 'nfs' || scheme === 'dav' || scheme === 'davs') {
    const targetPath = normalizePathForCompare(uriPath(uri))
    const key = scheme === 'nfs' ? 'share' : 'prefix'
    const byPath = candidates.filter((item) => normalizePathForCompare(item.params.get(key) ?? '') === targetPath)
    if (byPath.length > 0) {
      candidates = byPath
    }
  }

  return candidates[0].mount.path
}
