export type TopbarActionId =
  | 'open-settings'
  | 'open-shortcuts'
  | 'search'
  | 'toggle-hidden'
  | 'refresh'
  | 'about'

type TopbarViewMode = 'list' | 'grid'

type Deps = {
  openSettings: (initialFilter: string) => void
  isSearchMode: () => boolean
  setSearchMode: (value: boolean) => Promise<void>
  focusPathInput: () => void
  toggleShowHidden: () => void
  openAbout: () => void
  refresh: () => Promise<void>
  getViewMode: () => TopbarViewMode
  toggleViewMode: () => Promise<void>
}

export const createTopbarActions = (deps: Deps) => {
  const handleTopbarAction = (id: TopbarActionId) => {
    if (id === 'open-settings') {
      deps.openSettings('')
      return
    }
    if (id === 'open-shortcuts') {
      deps.openSettings('shortcut')
      return
    }
    if (id === 'search') {
      void (async () => {
        if (!deps.isSearchMode()) {
          await deps.setSearchMode(true)
        }
        deps.focusPathInput()
      })()
      return
    }
    if (id === 'toggle-hidden') {
      deps.toggleShowHidden()
      return
    }
    if (id === 'about') {
      deps.openAbout()
      return
    }
    if (id === 'refresh') {
      void deps.refresh()
    }
  }

  const handleTopbarViewModeChange = (nextMode: TopbarViewMode) => {
    if (nextMode === deps.getViewMode()) return
    void deps.toggleViewMode()
  }

  return {
    handleTopbarAction,
    handleTopbarViewModeChange,
  }
}
