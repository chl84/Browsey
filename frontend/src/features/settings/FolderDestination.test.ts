import { mount, tick, unmount } from 'svelte'
import { beforeEach, expect, it, vi } from 'vitest'
import FolderDestination from './FolderDestination.svelte'

const mocks = vi.hoisted(() => ({ homeDir: vi.fn(), invoke: vi.fn() }))
vi.mock('@tauri-apps/api/path', () => ({ homeDir: mocks.homeDir }))
vi.mock('@/shared/lib/tauri', () => ({ invoke: mocks.invoke }))
vi.mock('@/features/explorer', () => ({ parentPath: (path: string) => path.slice(0, path.lastIndexOf('/')) || '/' }))

beforeEach(() => {
  vi.clearAllMocks()
  mocks.invoke.mockImplementation(async (_cmd, { path }) => ({ current: path, entries: [] }))
})

it('loads the home folder on opening when the user has not chosen a destination', async () => {
  mocks.homeDir.mockResolvedValueOnce('/demo/home')
  const component = mount(FolderDestination, { target: document.body })
  try {
    await vi.waitFor(() => expect(document.querySelector<HTMLInputElement>('input')!.value).toBe('/demo/home'))
    expect(mocks.invoke).toHaveBeenCalledOnce()
  } finally {
    await unmount(component)
    document.body.innerHTML = ''
  }
})

it('preserves an unsubmitted destination draft when home lookup finishes', async () => {
  let settle!: (path: string) => void
  mocks.homeDir.mockReturnValueOnce(new Promise<string>(resolve => { settle = resolve }))
  const component = mount(FolderDestination, { target: document.body })
  try {
    await tick()
    const input = document.querySelector<HTMLInputElement>('input')!
    input.value = '/demo/recovery'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await tick()
    settle('/demo/home')
    await tick()
    await tick()
    expect(input.value).toBe('/demo/recovery')
    expect(mocks.invoke).not.toHaveBeenCalled()
  } finally {
    await unmount(component)
    document.body.innerHTML = ''
  }
})

it.each([false, true])('keeps an explicit destination when delayed home lookup settles (failure=%s)', async failure => {
  let settle!: () => void
  mocks.homeDir.mockReturnValueOnce(new Promise<string>((resolve, reject) => {
    settle = () => failure ? reject(new Error('Home unavailable')) : resolve('/demo/home')
  }))
  const component = mount(FolderDestination, { target: document.body })
  try {
    await tick()
    const input = document.querySelector<HTMLInputElement>('input[aria-label="Destination folder"]')!
    input.value = '/demo/recovery'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }))
    await tick()
    await tick()
    expect(input.value).toBe('/demo/recovery')
    settle()
    await tick()
    await tick()
    expect(input.value).toBe('/demo/recovery')
    expect(document.querySelector('[role="alert"]')).toBeNull()
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('list_dir', {
      path: '/demo/recovery', sort: { field: 'name', direction: 'asc' }, forceRefresh: true,
    })
  } finally {
    await unmount(component)
    document.body.innerHTML = ''
  }
})
