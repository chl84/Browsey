import type { ContextAction } from '../explorer/hooks/useContextMenus'
import { isExternallyOpenableUri, isMountUri, isMountableUri } from './uri'

export const buildNetworkEntryContextActions = (
  path: string,
  selectionCount: number,
): ContextAction[] | null => {
  if (isMountUri(path)) {
    const canOpen = selectionCount === 1 && (isMountableUri(path) || isExternallyOpenableUri(path))
    const openLabel = isExternallyOpenableUri(path) ? 'Open in Browser' : 'Connect'
    const actions: ContextAction[] = []
    if (canOpen) {
      actions.push({ id: 'open-network-target', label: openLabel })
    }
    actions.push({
      id: 'copy-network-address',
      label: selectionCount > 1 ? 'Copy Server Addresses' : 'Copy Server Address',
    })
    return actions
  }

  if (selectionCount !== 1) {
    return [
      { id: 'copy-path', label: 'Copy Mount Paths' },
      { id: 'properties', label: 'Properties' },
    ]
  }

  return [
    { id: 'open-network-target', label: 'Open' },
    { id: 'disconnect-network', label: 'Disconnect' },
    { id: 'divider-network-custom', label: '---' },
    { id: 'copy-path', label: 'Copy Mount Path' },
    { id: 'properties', label: 'Properties' },
  ]
}

export const networkBlankContextActions = (): ContextAction[] => [
  { id: 'refresh-network', label: 'Refresh Network' },
]
