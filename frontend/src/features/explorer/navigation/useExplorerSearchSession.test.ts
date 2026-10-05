import { describe, expect, it, vi } from 'vitest'
import { useExplorerSearchSession } from './useExplorerSearchSession'

type Mode = 'address' | 'filter'

const makeDeps = () => {
  let mode: Mode = 'address'
  let pathInput = ''
  let searchEnabled = false
  let currentPath = '/home/user'
  let searchAllowed = true

  const deps = {
    getCurrentPath: () => currentPath,
    getPathInput: () => pathInput,
    setPathInput: (value: string) => {
      pathInput = value
    },
    blurPathInput: vi.fn(),
    setMode: (next: Mode) => {
      mode = next
    },
    getMode: () => mode,
    isSearchSessionEnabled: () => searchEnabled,
    canUseSearch: () => searchAllowed,
    cancelSearch: vi.fn(),
    clearSearchResults: vi.fn(),
    setSearchMode: (value: boolean) => {
      searchEnabled = value
    },
    setFilterValue: vi.fn(),
    toggleMode: vi.fn(async (enabled: boolean) => {
      searchEnabled = enabled
    }),
    goToPath: vi.fn(async (_path: string) => {}),
    runSearch: vi.fn(),
  }

  return {
    deps,
    setPathInput: (value: string) => {
      pathInput = value
    },
    setSearchAllowed: (value: boolean) => {
      searchAllowed = value
    },
    setSearchEnabled: (value: boolean) => {
      searchEnabled = value
    },
    setCurrentPath: (value: string) => {
      currentPath = value
    },
    getMode: () => mode,
  }
}

describe('useExplorerSearchSession', () => {
  it('falls back to filter mode when search cannot be used', async () => {
    const { deps, setSearchAllowed, getMode } = makeDeps()
    setSearchAllowed(false)
    const session = useExplorerSearchSession(deps)

    await session.setSearchModeState(true)

    expect(getMode()).toBe('filter')
    expect(deps.setFilterValue).toHaveBeenCalledWith('')
    expect(deps.toggleMode).not.toHaveBeenCalled()
    expect(deps.runSearch).not.toHaveBeenCalled()
    expect(deps.getPathInput()).toBe('')
    expect(deps.isSearchSessionEnabled()).toBe(false)
  })

  it('enters a supported search and exits with the same folder and a cleared filter', async () => {
    const { deps, setCurrentPath, getMode } = makeDeps()
    setCurrentPath('/generated/owned')
    const session = useExplorerSearchSession(deps)
    await session.setSearchModeState(true)
    expect(deps.isSearchSessionEnabled()).toBe(true)
    await session.transitionToAddressMode()
    expect(deps.isSearchSessionEnabled()).toBe(false)
    expect(deps.getPathInput()).toBe('/generated/owned')
    expect(getMode()).toBe('address')
    expect(deps.setFilterValue).toHaveBeenLastCalledWith('')
  })

  it('marks search results stale when draft input diverges from submitted query', async () => {
    const { deps, setPathInput, setSearchEnabled } = makeDeps()
    const session = useExplorerSearchSession(deps)
    setSearchEnabled(true)

    setPathInput('invoice')
    session.submitSearch()
    expect(deps.runSearch).toHaveBeenCalledWith('invoice')

    setPathInput('invoice-final')
    session.syncSearchSessionWithInput()

    expect(deps.clearSearchResults).toHaveBeenCalledTimes(1)
    expect(deps.setFilterValue).toHaveBeenCalledWith('invoice')
    session.syncSearchSessionWithInput('  invoice-final  ', true)
    expect(deps.clearSearchResults).toHaveBeenCalledTimes(1)
  })

  it('uses explicit reactive input and mode values and never starts a search for an unsubmitted draft', () => {
    const { deps, setPathInput } = makeDeps()
    const session = useExplorerSearchSession(deps)
    setPathInput('alpha')
    session.submitSearch()
    deps.runSearch.mockClear()
    session.syncSearchSessionWithInput('unsubmitted', true)
    expect(deps.clearSearchResults).toHaveBeenCalledOnce()
    expect(deps.runSearch).not.toHaveBeenCalled()
    session.syncSearchSessionWithInput('/generated/owned', false)
    expect(deps.clearSearchResults).toHaveBeenCalledOnce()
  })

  it('navigates using trimmed path input on submitPath', () => {
    const { deps, setPathInput } = makeDeps()
    const session = useExplorerSearchSession(deps)
    setPathInput('   /tmp/workspace   ')

    session.submitPath()

    expect(deps.goToPath).toHaveBeenCalledWith('/tmp/workspace')
  })
})
