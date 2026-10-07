import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'
import { createPropertiesModal, type PropertiesState } from './propertiesModal'
import { normalizeError } from '@/shared/lib/error'

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}))

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

const computeDirStatsMock = vi.fn(async () => ({ total: 0, items: 0 }))

it.each([false, true])('keeps unmeasured cloud directory totals unknown (mixed=%s)', async mixed => {
  invokeMock.mockResolvedValue([])
  invokeMock.mockClear()
  computeDirStatsMock.mockClear()
  const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: vi.fn() })
  const folder = { ...makeEntry('rclone://test/owned/tree'), kind: 'dir' as const, size: 0 }
  const file = { ...makeEntry('rclone://test/owned/file.txt'), size: 41 }
  await modal.open(mixed ? [file, folder] : [folder])
  expect(get(modal.state)).toMatchObject({ size: null, itemCount: null })
  expect(computeDirStatsMock).not.toHaveBeenCalled()
  expect(invokeMock).not.toHaveBeenCalled()
})
const showToastMock = vi.fn()

const makeEntry = (path: string, kind: Entry['kind'] = 'file'): Entry => ({
  name: path.split('/').pop() ?? path,
  path,
  kind,
  iconId: 0,
})

const makeOpenState = (entry: Entry, count = 1): PropertiesState => ({
  open: true,
  partition: null,
  volumeUsage: { data: null, loading: false, error: null },
  entry,
  targets: [entry],
  mutationsLocked: false,
  count,
  size: null,
  itemCount: null,
  hidden: null,
  extraMetadataLoading: false,
  extraMetadataError: null,
  extraMetadata: null,
  extraMetadataPath: null,
  permissionsLoading: false,
  permissionsApplying: false,
  permissions: null,
  ownershipUsers: [],
  ownershipGroups: [],
  ownershipOptionsLoading: false,
  ownershipOptionsError: null,
  ownershipApplying: false,
  ownershipError: null,
})

describe('properties Hidden rename synchronization', () => {
  beforeEach(() => { vi.clearAllMocks(); invokeMock.mockReset() })

  it('reports only successful new paths and uses them for subsequent changes', async () => {
    const onHiddenChanged = vi.fn(async () => {})
    const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock, onHiddenChanged })
    const folder = makeEntry('/run/user/1000/gvfs/mtp:host=Phone/Storage/.test', 'dir')
    const failed = makeEntry('/run/user/1000/gvfs/mtp:host=Phone/Storage/.occupied', 'dir')
    modal.state.set({ ...makeOpenState(folder, 2), entry: null, targets: [folder, failed] })
    const path = folder.path.replace('/.test', '/test')
    invokeMock.mockResolvedValue({ per_item: [
      { path: folder.path, ok: true, new_path: path },
      { path: failed.path, ok: false, new_path: failed.path },
    ], failures: 1, unexpected_failures: 0 })
    await modal.toggleHidden(false)
    expect(onHiddenChanged).toHaveBeenCalledWith([{ path: folder.path, entry: { ...folder, path, name: 'test', nameLower: 'test', hidden: false } }])
    expect(get(modal.state).targets.map(entry => entry.path)).toEqual([path, failed.path])
    await modal.toggleHidden(true)
    expect(invokeMock).toHaveBeenLastCalledWith('set_hidden', { paths: [path, failed.path], hidden: true })
  })

  it('synchronizes a completed filesystem rename after the dialog closes', async () => {
    let finish!: (value: unknown) => void
    invokeMock.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const onHiddenChanged = vi.fn(async () => {})
    const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock, onHiddenChanged })
    modal.state.set(makeOpenState(makeEntry('/owned/.test')))
    const pending = modal.toggleHidden(false)
    modal.close()
    finish({ per_item: [{ path: '/owned/.test', ok: true, new_path: '/owned/test' }], failures: 0, unexpected_failures: 0 })
    await pending
    expect(onHiddenChanged).toHaveBeenCalledOnce()
    expect(get(modal.state)).toMatchObject({ open: false, targets: [] })
  })

  it('preserves the successful rename and distinguishes refresh failure from mutation failure', async () => {
    const onHiddenChanged = vi.fn(async () => { throw Error('MTP refresh failed') })
    const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock, onHiddenChanged })
    modal.state.set(makeOpenState(makeEntry('/owned/.test')))
    invokeMock.mockResolvedValue({ per_item: [{ path: '/owned/.test', ok: true, new_path: '/owned/test' }], failures: 0, unexpected_failures: 0 })
    await modal.toggleHidden(false)
    expect(get(modal.state).entry?.path).toBe('/owned/test')
    expect(invokeMock).toHaveBeenCalledOnce()
    expect(showToastMock).toHaveBeenCalledWith('Hidden state changed, but refresh failed. Press F5 to refresh.')
  })
})

describe('properties modal copyParentFolder', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockReset()
  })

  it('copies full local parent path', async () => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: vi.fn(() => true),
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('/home/chris/docs/report.txt')))

    await modal.copyParentFolder()

    expect(writeText).toHaveBeenCalledWith('/home/chris/docs')
    expect(showToastMock).toHaveBeenCalledWith('Parent folder copied', 1500)
  })

  it('copies full cloud parent path', async () => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: vi.fn(() => true),
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('rclone://work/docs/report.txt')))

    await modal.copyParentFolder()

    expect(writeText).toHaveBeenCalledWith('rclone://work/docs')
    expect(showToastMock).toHaveBeenCalledWith('Parent folder copied', 1500)
  })

  it.each([
    ['/file.txt', '/'],
    ['C:/file.txt', 'C:/'],
  ])('handles edge paths without crashing (%s)', async (entryPath, expectedParent) => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: vi.fn(() => true),
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry(entryPath)))

    await modal.copyParentFolder()

    expect(writeText).toHaveBeenCalledWith(expectedParent)
  })

  it('falls back to execCommand when clipboard API fails', async () => {
    const writeText = vi.fn(async () => {
      throw new Error('write denied')
    })
    const execCommand = vi.fn(() => true)
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('/home/chris/docs/report.txt')))

    await modal.copyParentFolder()

    expect(execCommand).toHaveBeenCalledWith('copy')
    expect(showToastMock).toHaveBeenCalledWith('Parent folder copied', 1500)
  })

  it('shows copy failure when clipboard API and fallback both fail', async () => {
    const writeText = vi.fn(async () => {
      throw new Error('permission denied')
    })
    const execCommand = vi.fn(() => false)
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: execCommand,
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('/home/chris/docs/report.txt')))

    await modal.copyParentFolder()

    expect(showToastMock).toHaveBeenCalledWith('Copy failed: permission denied')
  })

  it('is a no-op for multi-selection state', async () => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: vi.fn(() => true),
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('/home/chris/docs/report.txt'), 2))

    await modal.copyParentFolder()

    expect(writeText).not.toHaveBeenCalled()
    expect(showToastMock).not.toHaveBeenCalled()
  })

  it('shows a user-facing ownership error for helper failures', async () => {
    invokeMock.mockRejectedValueOnce({
      code: 'helper_protocol_error',
      message: 'unexpected helper response payload',
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set(makeOpenState(makeEntry('/home/chris/docs/report.txt')))

    await modal.setOwnership('root', '')

    expect(get(modal.state).ownershipError).toBe(
      'Browsey could not complete the privileged permissions step.',
    )
  })

  it('shows a user-facing toast when permission updates fail', async () => {
    invokeMock.mockRejectedValueOnce({
      code: 'helper_protocol_error',
      message: 'unexpected helper response payload',
    })

    const modal = createPropertiesModal({
      computeDirStats: computeDirStatsMock,
      showToast: showToastMock,
    })
    modal.state.set({
      ...makeOpenState(makeEntry('/home/chris/docs/report.txt')),
      permissions: {
        accessSupported: true,
        ownershipSupported: true,
        ownerName: 'chris',
        groupName: 'chris',
        owner: { read: false, write: true, exec: false },
        group: { read: false, write: false, exec: false },
        other: { read: false, write: false, exec: false },
      },
    })

    modal.toggleAccess('owner', 'read', true)

    await vi.waitFor(() => {
      expect(showToastMock).toHaveBeenCalledWith(
        'Permissions update failed: Browsey could not complete the privileged permissions step.',
      )
    })
  })
})

describe('Properties error-shape compatibility', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockReset()
  })

  it.each([
    ['nested helper', { error: { code: 'helper_start_failed', message: 'Private helper diagnostic' } }, 'Browsey could not complete the privileged permissions step.'],
    ['JSON authentication', JSON.stringify({ code: 'authentication_cancelled', message: 'Authentication diagnostic' }), 'Authentication was cancelled.'],
    ['normalized JSON authentication', normalizeError(JSON.stringify({ code: 'authentication_cancelled', message: 'Authentication diagnostic' })), 'Authentication was cancelled.'],
    ['normalized nested ownership', normalizeError({ error: { code: 'elevated_required', message: 'Elevated diagnostic' } }), 'Permission denied. Changing owner or group requires elevated privileges.'],
    ['Error with code', Object.assign(new Error('Owner diagnostic'), { code: ' permission_denied ' }), 'Permission denied. Changing owner or group requires elevated privileges.'],
    ['metadata', { code: 'metadata_read_failed', message: 'Metadata diagnostic' }, 'Browsey could not read the current permissions.'],
    ['snapshot', { code: 'post_change_snapshot_failed', message: 'Snapshot diagnostic' }, 'Permissions were changed, but Browsey could not verify the final state. Refresh and review the item before continuing.'],
    ['rollback', { code: 'rollback_failed', message: 'Rollback diagnostic' }, 'Some permission changes could not be rolled back cleanly. Refresh and review the item before continuing.'],
    ['read-only', { code: 'read_only_filesystem', message: 'Mount diagnostic' }, 'This location is read-only.'],
    ['cause string', { cause: 'Original cause' }, 'Original cause'],
    ['nested cause', { error: { cause: 'Nested cause' } }, 'Nested cause'],
    ['untyped prose', 'permission_denied', 'permission_denied'],
  ])('preserves ownership feedback for %s', async (_shape, error, expected) => {
    invokeMock.mockRejectedValueOnce(error)
    const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock })
    modal.state.set(makeOpenState(makeEntry('/mock/file.txt')))
    await modal.setOwnership('test-user', 'test-group')
    expect(get(modal.state).ownershipError).toBe(expected)
    expect(get(modal.state).ownershipApplying).toBe(false)
    expect(invokeMock).toHaveBeenCalledOnce()
  })

  it.each([
    ['helper', normalizeError({ error: { code: 'helper_io_error', message: 'Helper diagnostic' } }), 'Browsey could not complete the privileged permissions step.'],
    ['cancelled', normalizeError(JSON.stringify({ code: 'authentication_cancelled', message: 'Cancelled diagnostic' })), 'Authentication was cancelled.'],
    ['snapshot', { error: { code: 'post_change_snapshot_failed', message: 'Snapshot diagnostic' } }, 'Permissions were changed, but Browsey could not verify the final state. Refresh and review the item before continuing.'],
    ['read-only', { code: 'read_only_filesystem', message: 'Read-only diagnostic' }, 'This location is read-only.'],
  ])('preserves permission feedback and previous UI state for %s', async (_shape, error, expected) => {
    invokeMock.mockRejectedValueOnce(error)
    const modal = createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock })
    modal.state.set({
      ...makeOpenState(makeEntry('/mock/file.txt')),
      permissions: {
        accessSupported: true, ownershipSupported: true, ownerName: 'test-user', groupName: 'test-group',
        owner: { read: false, write: true, exec: false }, group: null, other: null,
      },
    })
    modal.toggleAccess('owner', 'read', true)
    await vi.waitFor(() => expect(showToastMock).toHaveBeenCalledWith(`Permissions update failed: ${expected}`))
    expect(get(modal.state).permissions?.owner?.read).toBe(false)
    expect(get(modal.state).permissionsApplying).toBe(false)
    expect(invokeMock).toHaveBeenCalledOnce()
  })
})

describe('mount-managed permissions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockReset()
  })

  const payload = {
    restriction: 'write_protection', access_supported: true, executable_supported: false,
    ownership_supported: false, read_only: false, executable: true,
    owner: { read: true, write: true, exec: true },
    group: { read: true, write: true, exec: true },
    other: { read: true, write: false, exec: true },
  }
  const createModal = () => createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock })

  it('loads FAT restrictions and ignores unsupported checkbox requests without invoking mutations', async () => {
    invokeMock.mockResolvedValue(payload)
    const modal = createModal()
    await modal.open([makeEntry('/media/USB/file.pdf')])
    await vi.waitFor(() => expect(get(modal.state).permissions?.restriction).toBe('write_protection'))
    modal.toggleAccess('owner', 'read', false)
    modal.toggleAccess('owner', 'exec', false)
    modal.toggleAccess('group', 'write', false)
    modal.toggleAccess('other', 'write', false)
    expect(invokeMock.mock.calls.some(([cmd]) => cmd === 'set_permissions')).toBe(false)
    expect(get(modal.state).permissions?.owner?.read).toBe(true)
    expect(invokeMock.mock.calls.some(([cmd]) => cmd === 'list_ownership_principals')).toBe(false)
  })

  it('keeps owner Write available and refreshes all scopes after a global write-protection toggle', async () => {
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'set_permissions' ? {
      ...payload, read_only: true,
      owner: { ...payload.owner, write: false }, group: { ...payload.group, write: false },
    } : payload)
    const modal = createModal()
    await modal.open([makeEntry('/media/USB/file.pdf')])
    await vi.waitFor(() => expect(get(modal.state).permissions?.restriction).toBe('write_protection'))
    modal.toggleAccess('owner', 'write', false)
    await vi.waitFor(() => expect(get(modal.state).permissions?.group?.write).toBe(false))
    expect(invokeMock).toHaveBeenCalledWith('set_permissions', { paths: ['/media/USB/file.pdf'], owner: { write: false } })
    expect(get(modal.state).permissions).toMatchObject({ restriction: 'write_protection', owner: { write: false } })
  })

  it('carries batch restrictions into mixed selections and blocks all edits for mount-managed batches', async () => {
    invokeMock.mockResolvedValue({ aggregate: { ...payload, restriction: 'mount_managed' }, per_item: [], failures: 0, unexpected_failures: 0 })
    const modal = createModal()
    await modal.open([makeEntry('/media/USB/file.pdf'), makeEntry('/media/USB/folder', 'dir')])
    await vi.waitFor(() => expect(get(modal.state).permissions?.restriction).toBe('mount_managed'))
    modal.toggleAccess('owner', 'write', false)
    expect(invokeMock.mock.calls.some(([cmd]) => cmd === 'set_permissions')).toBe(false)
  })

  it.each(['read_only', 'network_managed'])('prevents permission and ownership IPC mutations for %s', async (restriction) => {
    invokeMock.mockResolvedValue({ ...payload, restriction })
    const modal = createModal()
    await modal.open([makeEntry('/volume/file.pdf')])
    await vi.waitFor(() => expect(get(modal.state).permissions?.restriction).toBe(restriction))
    for (const scope of ['owner', 'group', 'other'] as const) {
      for (const key of ['read', 'write', 'exec'] as const) modal.toggleAccess(scope, key, false)
    }
    await modal.setOwnership('another-user', 'another-group')
    expect(invokeMock.mock.calls.some(([cmd]) => ['set_permissions', 'set_ownership'].includes(cmd))).toBe(false)
  })

  it('restores checkbox state and reports a verification failure instead of showing an ignored remote change as applied', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'set_permissions') throw { code: 'permissions_update_failed', message: 'The filesystem did not apply the requested permissions.' }
      return { ...payload, restriction: null, ownership_supported: true }
    })
    const modal = createModal()
    await modal.open([makeEntry('/network/file.pdf')])
    await vi.waitFor(() => expect(get(modal.state).permissions?.owner?.write).toBe(true))
    modal.toggleAccess('owner', 'write', false)
    await vi.waitFor(() => expect(showToastMock).toHaveBeenCalledWith('Permissions update failed: The filesystem did not apply the requested permissions.'))
    expect(get(modal.state).permissions?.owner?.write).toBe(true)
  })
})

describe('USB properties', () => {
  const partition = { label: 'USB', path: '/media/chris/USB', fs: 'btrfs', removable: true }
  const permissions = {
    access_supported: true, ownership_supported: true, owner_name: 'chris', group_name: 'users',
    owner: { read: true, write: true, exec: true },
    group: { read: true, write: false, exec: true },
    other: { read: true, write: false, exec: true },
  }
  const createModal = () => createPropertiesModal({ computeDirStats: computeDirStatsMock, showToast: showToastMock })

  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockReset()
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'get_permissions' ? permissions : cmd === 'get_volume_usage' ? null : [])
  })

  it('reads mount-root permissions without scanning contents or changing anything', async () => {
    const modal = createModal()
    await modal.openPartition(partition)
    await vi.waitFor(() => expect(get(modal.state).permissions?.ownerName).toBe('chris'))
    expect(get(modal.state)).toMatchObject({ partition, size: null, itemCount: null, mutationsLocked: false })
    expect(invokeMock).toHaveBeenCalledWith('get_permissions', { path: partition.path })
    expect(computeDirStatsMock).not.toHaveBeenCalled()
    await modal.toggleHidden(true)
    modal.loadExtraIfNeeded()
    expect(invokeMock.mock.calls.every(([cmd]) => ['get_permissions', 'list_ownership_principals', 'get_volume_usage'].includes(cmd))).toBe(true)
  })

  it('does not mount, inspect, or mutate an unmounted device', async () => {
    const modal = createModal()
    await modal.openPartition({ ...partition, path: 'usb-volume:///dev/sdz1' })
    expect(get(modal.state).mutationsLocked).toBe(true)
    await modal.toggleHidden(true)
    await modal.setOwnership('chris', 'users')
    modal.toggleAccess('owner', 'write', true)
    modal.loadExtraIfNeeded()
    expect(invokeMock).not.toHaveBeenCalled()
    expect(computeDirStatsMock).not.toHaveBeenCalled()
  })

  it('permission edits target only the mount root', async () => {
    const modal = createModal()
    await modal.openPartition(partition)
    modal.toggleAccess('other', 'write', true)
    expect(invokeMock).toHaveBeenCalledWith('set_permissions', {
      paths: [partition.path], other: { write: true },
    })
  })

  it.each(['/media/chris/USB', 'usb-volume:///dev/sdz1'])('copies the drive path, not its parent (%s)', async (path) => {
    const writeText = vi.fn(async () => {})
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
    const modal = createModal()
    await modal.openPartition({ ...partition, path })
    await modal.copyParentFolder()
    expect(writeText).toHaveBeenCalledWith(path.replace('usb-volume://', ''))
  })

  it('clears drive context when opening a normal file', async () => {
    const modal = createModal()
    await modal.openPartition(partition)
    await modal.open([makeEntry('/home/chris/file.txt')])
    expect(get(modal.state).partition).toBeNull()
  })

  it.each(['mtp://Phone_A/', '/run/user/1000/gvfs/mtp:host=Phone_A'])('does not expose Unix permission edits for phones (%s)', async (path) => {
    const modal = createModal()
    await modal.openPartition({ label: 'Phone', path, fs: 'mtp', removable: true })
    expect(get(modal.state).mutationsLocked).toBe(true)
    await modal.setOwnership('root', 'root')
    modal.toggleAccess('other', 'write', true)
    await modal.toggleHidden(true)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('ignores a pending permissions reply after the dialog closes', async () => {
    let resolve!: (value: typeof permissions) => void
    invokeMock.mockImplementation((cmd: string) => cmd === 'get_permissions'
      ? new Promise((done) => { resolve = done }) : Promise.resolve(null))
    const modal = createModal()
    await modal.openPartition(partition)
    modal.close()
    resolve(permissions)
    await Promise.resolve()
    expect(get(modal.state)).toMatchObject({ open: false, partition: null, permissions: null })
  })

  const usage = { totalBytes: 32_000_000_000, usedBytes: 8_000_000_000, freeBytes: 24_000_000_000, reservedBytes: 0 }

  it('loads filesystem statistics for a fixed volume without scanning files', async () => {
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'get_permissions' ? permissions : cmd === 'get_volume_usage' ? usage : [])
    const modal = createModal()
    await modal.openPartition({ label: '/', path: '/', fs: 'btrfs', removable: false })
    await vi.waitFor(() => expect(get(modal.state).volumeUsage).toEqual({ data: usage, loading: false, error: null }))
    expect(invokeMock).toHaveBeenCalledWith('get_volume_usage', { path: '/' })
    expect(computeDirStatsMock).not.toHaveBeenCalled()
  })

  it('keeps Properties open with a concise usage error when reading fails', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_volume_usage') throw new Error('Device disconnected')
      return cmd === 'get_permissions' ? permissions : []
    })
    const modal = createModal()
    await modal.openPartition(partition)
    await vi.waitFor(() => expect(get(modal.state).volumeUsage).toEqual({ data: null, loading: false, error: 'Could not read storage usage.' }))
    expect(get(modal.state).open).toBe(true)
    expect(showToastMock).not.toHaveBeenCalled()
  })

  it('ignores a pending usage reply after Properties closes', async () => {
    let resolve!: (value: typeof usage) => void
    invokeMock.mockImplementation((cmd: string) => cmd === 'get_volume_usage'
      ? new Promise((done) => { resolve = done }) : Promise.resolve(cmd === 'get_permissions' ? permissions : []))
    const modal = createModal()
    await modal.openPartition(partition)
    expect(get(modal.state).volumeUsage.loading).toBe(true)
    modal.close()
    resolve(usage)
    await Promise.resolve()
    expect(get(modal.state).volumeUsage).toEqual({ data: null, loading: false, error: null })
  })

  it('ignores old usage when opening a different volume', async () => {
    let resolveOld!: (value: typeof usage) => void
    const newer = { ...usage, usedBytes: 16_000_000_000, freeBytes: 16_000_000_000 }
    invokeMock.mockImplementation((cmd: string, args: { path?: string }) => {
      if (cmd === 'get_volume_usage') return args.path === partition.path
        ? new Promise((done) => { resolveOld = done }) : Promise.resolve(newer)
      return Promise.resolve(cmd === 'get_permissions' ? permissions : [])
    })
    const modal = createModal()
    await modal.openPartition(partition)
    await modal.openPartition({ label: '/', path: '/', fs: 'btrfs' })
    await vi.waitFor(() => expect(get(modal.state).volumeUsage.data).toEqual(newer))
    resolveOld(usage)
    await Promise.resolve()
    expect(get(modal.state).partition?.path).toBe('/')
    expect(get(modal.state).volumeUsage.data).toEqual(newer)
  })

  it('does not probe virtual network addresses for local storage statistics', async () => {
    const modal = createModal()
    await modal.openPartition({ label: 'NAS', path: 'smb://nas/share', fs: 'smb' })
    expect(get(modal.state).volumeUsage).toEqual({ data: null, loading: false, error: null })
    expect(invokeMock).not.toHaveBeenCalled()
  })
})
