import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'
import { createPropertiesModal, type PropertiesState } from './propertiesModal'

const computeDirStatsMock = vi.fn(async () => ({ total: 0, items: 0 }))
const showToastMock = vi.fn()

const makeEntry = (path: string, kind: Entry['kind'] = 'file'): Entry => ({
  name: path.split('/').pop() ?? path,
  path,
  kind,
  iconId: 0,
})

const makeOpenState = (entry: Entry, count = 1): PropertiesState => ({
  open: true,
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

describe('properties modal copyParentFolder', () => {
  beforeEach(() => {
    vi.clearAllMocks()
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
})
