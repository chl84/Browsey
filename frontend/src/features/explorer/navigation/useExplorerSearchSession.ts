type Mode = 'address' | 'filter'

type SearchSession = {
  submittedQuery: string
  clearedDraftQuery: string | null
}

type ExitOptions = {
  reloadOnDisable?: boolean
  skipToggle?: boolean
}

type AddressTransitionOptions = {
  path?: string
  blur?: boolean
  reloadOnDisable?: boolean
  skipToggle?: boolean
}

type Deps = {
  getCurrentPath: () => string
  getPathInput: () => string
  setPathInput: (value: string) => void
  blurPathInput: () => void
  setMode: (next: Mode) => void
  getMode: () => Mode
  isSearchSessionEnabled: () => boolean
  canUseSearch: () => boolean
  cancelSearch: () => void
  setSearchMode: (value: boolean) => void
  setFilterValue: (value: string) => void
  toggleMode: (enabled: boolean, opts?: { reloadOnDisable?: boolean }) => Promise<void>
  goToPath: (path: string) => Promise<void>
  runSearch: (query: string) => unknown
}

export const useExplorerSearchSession = (deps: Deps) => {
  let searchSession: SearchSession = {
    submittedQuery: '',
    clearedDraftQuery: null,
  }

  const normalizeSearchQuery = (value: string) => value.trim()

  const patchSearchSession = (patch: Partial<SearchSession>) => {
    const next: SearchSession = {
      ...searchSession,
      ...patch,
    }
    if (
      next.submittedQuery === searchSession.submittedQuery &&
      next.clearedDraftQuery === searchSession.clearedDraftQuery
    ) {
      return
    }
    searchSession = next
  }

  const resetSearchSession = () => {
    patchSearchSession({
      submittedQuery: '',
      clearedDraftQuery: null,
    })
  }

  const markSearchResultsStale = (draftQuery: string) => {
    if (searchSession.clearedDraftQuery === draftQuery) return
    deps.cancelSearch()
    deps.setFilterValue(searchSession.submittedQuery)
    patchSearchSession({ clearedDraftQuery: draftQuery })
  }

  const exitSearchSession = async (options: ExitOptions = {}) => {
    const { reloadOnDisable = true, skipToggle = false } = options
    if (skipToggle) {
      deps.cancelSearch()
      deps.setSearchMode(false)
    } else if (deps.isSearchSessionEnabled()) {
      await deps.toggleMode(false, { reloadOnDisable })
    }
    deps.setFilterValue('')
    resetSearchSession()
  }

  const transitionToAddressMode = async (options: AddressTransitionOptions = {}) => {
    const {
      path = deps.getCurrentPath(),
      blur = false,
      reloadOnDisable = true,
      skipToggle = false,
    } = options
    deps.setMode('address')
    await exitSearchSession({ reloadOnDisable, skipToggle })
    deps.setPathInput(path)
    if (blur) {
      deps.blurPathInput()
    }
  }

  const transitionToFilterMode = async (path = '') => {
    deps.setMode('filter')
    await exitSearchSession({ reloadOnDisable: false })
    deps.setPathInput(path)
  }

  const setSearchModeState = async (value: boolean) => {
    if (value && !deps.canUseSearch()) {
      await transitionToFilterMode('')
      return
    }
    if (!value) {
      await transitionToAddressMode({ path: deps.getCurrentPath() })
      return
    }
    deps.setPathInput('')
    resetSearchSession()
    await deps.toggleMode(true)
    deps.setMode('address')
  }

  const submitPath = () => {
    const trimmed = deps.getPathInput().trim()
    if (!trimmed) return
    void deps.goToPath(trimmed)
  }

  const submitSearch = () => {
    const currentInput = deps.getPathInput()
    patchSearchSession({
      submittedQuery: normalizeSearchQuery(currentInput),
      clearedDraftQuery: null,
    })
    void deps.runSearch(currentInput)
  }

  const enterAddressMode = async () => {
    await transitionToAddressMode({ path: deps.getCurrentPath() })
  }

  const syncSearchSessionWithInput = () => {
    if (!deps.isSearchSessionEnabled()) {
      resetSearchSession()
      return
    }
    const draftQuery = normalizeSearchQuery(deps.getPathInput())
    const stale = draftQuery !== searchSession.submittedQuery
    if (stale) {
      markSearchResultsStale(draftQuery)
    } else {
      patchSearchSession({ clearedDraftQuery: null })
    }
  }

  const resetInputModeForNavigation = (
    returnToBreadcrumb = false,
    currentPath = deps.getCurrentPath(),
  ) => {
    void transitionToAddressMode({
      path: currentPath,
      blur: returnToBreadcrumb,
      reloadOnDisable: false,
      skipToggle: true,
    })
  }

  const isTypingFilterOrSearch = () => deps.getMode() === 'filter' || deps.isSearchSessionEnabled()

  return {
    transitionToAddressMode,
    transitionToFilterMode,
    setSearchModeState,
    submitPath,
    submitSearch,
    enterAddressMode,
    syncSearchSessionWithInput,
    resetInputModeForNavigation,
    isTypingFilterOrSearch,
  }
}
