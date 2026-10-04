import { beforeEach, describe, expect, it, vi } from 'vitest'
import { buildNetworkEntryContextActions } from './contextMenu'

const { classifyNetworkUri, listSavedNetworkConnections, resolveMountedPathForUri } = vi.hoisted(() => ({
  classifyNetworkUri: vi.fn(), listSavedNetworkConnections: vi.fn(), resolveMountedPathForUri: vi.fn(),
}))
vi.mock('./services', () => ({ classifyNetworkUri, listSavedNetworkConnections, resolveMountedPathForUri }))

const address = 'sftp://alice@server/'
const actions = async (path = address, count = 1) =>
  (await buildNetworkEntryContextActions(path, count))?.map(action => action.id)

describe('saved network connection actions', () => {
  beforeEach(() => {
    vi.resetAllMocks()
    classifyNetworkUri.mockResolvedValue({ kind: 'mountable', scheme: 'sftp', normalizedUri: address })
    listSavedNetworkConnections.mockResolvedValue([{ uri: address, label: 'SFTP (alice@server)' }])
    resolveMountedPathForUri.mockResolvedValue(null)
  })

  it('allows reconnecting and forgetting a saved offline server', async () => {
    expect(await actions()).toEqual(['open-network-target', 'copy-network-address', 'forget-network-connection'])
  })

  it('retains Disconnect when a saved server is mounted', async () => {
    resolveMountedPathForUri.mockResolvedValue('/run/user/1000/gvfs/sftp:host=server,user=alice')
    expect(await actions()).toEqual(['open-network-target', 'copy-network-address', 'disconnect-network', 'forget-network-connection'])
    expect(resolveMountedPathForUri).toHaveBeenCalledWith(address)
  })

  it('does not confuse another account with a saved connection', async () => {
    expect(await actions('sftp://bob@server/')).toEqual(['open-network-target', 'copy-network-address'])
    expect(resolveMountedPathForUri).not.toHaveBeenCalled()
  })

  it('does not offer a single-target Forget action for multiple selections', async () => {
    expect(await actions(address, 2)).toEqual(['copy-network-address'])
    expect(listSavedNetworkConnections).not.toHaveBeenCalled()
  })

  it('keeps Connect available if history or mount-path lookup fails', async () => {
    listSavedNetworkConnections.mockRejectedValueOnce(new Error('Database unavailable'))
    expect(await actions()).toEqual(['open-network-target', 'copy-network-address'])
    resolveMountedPathForUri.mockRejectedValueOnce(new Error('Mount unavailable'))
    expect(await actions()).toContain('forget-network-connection')
  })

  it('does not add saved-server actions to cloud entries or browser links', async () => {
    expect(await actions('rclone://remote/')).not.toContain('forget-network-connection')
    classifyNetworkUri.mockResolvedValue({ kind: 'external', scheme: 'https' })
    expect(await actions('https://server/')).toEqual(['open-network-target', 'copy-network-address'])
    expect(listSavedNetworkConnections).not.toHaveBeenCalled()
  })
})
