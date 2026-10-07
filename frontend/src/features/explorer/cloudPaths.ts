/** Backend-issued Drive addresses carry identity; labels remain ordinary paths. */
const parseDrive = (path: string) => {
  if (!path.includes('//gdrive/')) return null
  const match = /^rclone:\/\/([^/]+)\/\/gdrive\/(.+)$/.exec(path)
  if (!match) return null
  const segments = match[2].split('/')
  const names: string[] = []
  for (const segment of segments) {
    const index = segment.indexOf('~')
    if (index < 0) return null
    try { names.push(decodeURIComponent(segment.slice(index + 1).replace(/\+/g, ' '))) } catch { return null }
  }
  return { root: `rclone://${match[1]}`, segments, names }
}

export const cloudDisplayPath = (path: string): string => {
  const drive = parseDrive(path)
  return drive ? `${drive.root}/${drive.names.join('/')}` : path
}
export const cloudLeafName = (path: string): string => {
  const drive = parseDrive(path)
  return drive?.names.at(-1) ?? path.split(/[\\/]+/).filter(Boolean).at(-1) ?? path
}
export const cloudParentPath = (path: string): string => {
  const drive = parseDrive(path)
  if (drive) return drive.segments.length === 1 ? drive.root : `${drive.root}//gdrive/${drive.segments.slice(0, -1).join('/')}`
  const index = path.lastIndexOf('/')
  return index > 'rclone://'.length ? path.slice(0, index) : path
}
export const joinCloudPath = (dir: string, name: string): string => {
  const drive = parseDrive(dir)
  const leaf = drive ? `~${encodeURIComponent(name).replace(/~/g, '%7E')}` : name
  return `${dir.replace(/\/+$/, '')}/${leaf}`
}
export const cloudBreadcrumbs = (path: string): Array<{ label: string; path: string }> | null => {
  if (!path.startsWith('rclone://')) return null
  const drive = parseDrive(path)
  if (drive) return [
    { label: drive.root.slice('rclone://'.length), path: drive.root },
    ...drive.names.map((name, index) => ({ label: name, path: `${drive.root}//gdrive/${drive.segments.slice(0, index + 1).join('/')}` })),
  ]
  const parts = path.slice('rclone://'.length).split('/').filter(Boolean)
  if (!parts.length) return [{ label: path, path }]
  return parts.map((label, index) => ({ label, path: `rclone://${parts.slice(0, index + 1).join('/')}` }))
}
