type Deps = {
  hasActiveSearchSortTarget: () => boolean
  sortActiveSearchEntries: () => void
  getCurrentWhere: () => string
  loadRecentForSort: () => Promise<void>
  loadStarredForSort: () => Promise<void>
  loadNetworkForSort: () => Promise<void>
  loadTrashForSort: () => Promise<void>
  loadDirectoryForSort: (where: string) => Promise<void>
}

export const createSortRefreshDispatcher = (deps: Deps) => {
  return async () => {
    if (deps.hasActiveSearchSortTarget()) {
      deps.sortActiveSearchEntries()
      return
    }

    const where = deps.getCurrentWhere()
    if (where === 'Recent') {
      await deps.loadRecentForSort()
    } else if (where === 'Starred') {
      await deps.loadStarredForSort()
    } else if (where === 'Network') {
      await deps.loadNetworkForSort()
    } else if (where === 'Trash') {
      await deps.loadTrashForSort()
    } else {
      await deps.loadDirectoryForSort(where)
    }
  }
}

