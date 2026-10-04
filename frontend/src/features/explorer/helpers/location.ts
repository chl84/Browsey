/** GVFS paths are remote filesystems even though they look like local paths. */
export const isGvfsPath = (path: string | null | undefined): path is string =>
  Boolean(path && path.includes('/run/user/') && path.includes('/gvfs/'))
