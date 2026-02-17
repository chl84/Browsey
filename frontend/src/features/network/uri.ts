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
    case 'ftps':
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
