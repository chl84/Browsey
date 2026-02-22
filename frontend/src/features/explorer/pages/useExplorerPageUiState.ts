type Params = {
  getSettingsOpen: () => boolean
  setSettingsOpen: (next: boolean) => void
  setSettingsInitialFilter: (next: string) => void
  setAboutOpen: (next: boolean) => void
}

export const useExplorerPageUiState = (params: Params) => {
  const openSettings = (initialFilter: string = '') => {
    params.setSettingsInitialFilter(initialFilter)
    params.setSettingsOpen(true)
  }

  const closeSettings = () => {
    params.setSettingsOpen(false)
  }

  const toggleSettings = () => {
    if (params.getSettingsOpen()) {
      closeSettings()
      return
    }
    openSettings('')
  }

  const openAbout = () => {
    params.setAboutOpen(true)
  }

  const closeAbout = () => {
    params.setAboutOpen(false)
  }

  return {
    openSettings,
    closeSettings,
    toggleSettings,
    openAbout,
    closeAbout,
  }
}
