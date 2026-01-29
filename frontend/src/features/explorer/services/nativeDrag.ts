import { startDrag } from '@crabnebula/tauri-plugin-drag'
import { resourceDir, join } from '@tauri-apps/api/path'

const resolveIcon = async (fallbackPath: string) => {
  try {
    const resDir = await resourceDir()
    const candidate = await join(resDir, 'icons', 'icon.png')
    return candidate
  } catch {
    // ignore, fall through
  }
  return fallbackPath
}

export const startNativeFileDrag = async (paths: string[], mode: 'copy' | 'move' = 'copy') => {
  if (!paths || paths.length === 0) return false
  const iconPath = await resolveIcon(paths[0])
  try {
    await startDrag({
      item: paths,
      icon: iconPath,
      mode,
    })
    return true
  } catch (err) {
    console.error('native drag failed', err)
    return false
  }
}
