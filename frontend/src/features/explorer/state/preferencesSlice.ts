import { get } from 'svelte/store'
import type { DefaultSortField, Density, SortDirection } from '../model/types'
import {
  loadArchiveLevel,
  loadArchiveName,
  loadConfirmDelete,
  loadDensity,
  loadDoubleClickMs,
  loadFfmpegPath,
  loadFoldersFirst,
  loadHardwareAcceleration,
  loadHiddenFilesLast,
  loadHighContrast,
  loadMountsPollMs,
  loadOpenDestAfterExtract,
  loadShowHidden,
  loadSortDirection,
  loadSortField,
  loadStartDir,
  loadThumbCacheMb,
  loadVideoThumbs,
  storeArchiveLevel,
  storeArchiveName,
  storeConfirmDelete,
  storeDensity,
  storeDoubleClickMs,
  storeFfmpegPath,
  storeFoldersFirst,
  storeHardwareAcceleration,
  storeHiddenFilesLast,
  storeHighContrast,
  storeMountsPollMs,
  storeOpenDestAfterExtract,
  storeShowHidden,
  storeSortDirection,
  storeSortField,
  storeStartDir,
  storeThumbCacheMb,
  storeVideoThumbs,
} from '../services/settings.service'
import type { ExplorerStores } from './stores'

type PreferenceSliceDeps = Pick<
  ExplorerStores,
  | 'showHidden'
  | 'hiddenFilesLast'
  | 'highContrast'
  | 'foldersFirst'
  | 'startDirPref'
  | 'confirmDelete'
  | 'sortField'
  | 'sortDirection'
  | 'sortFieldPref'
  | 'sortDirectionPref'
  | 'archiveName'
  | 'archiveLevel'
  | 'openDestAfterExtract'
  | 'videoThumbs'
  | 'hardwareAcceleration'
  | 'ffmpegPath'
  | 'thumbCacheMb'
  | 'mountsPollMs'
  | 'doubleClickMs'
  | 'density'
>

type PreferenceSliceOptions = {
  clearFacetCache: () => void
  refreshForSort: () => Promise<void>
}

export const createPreferenceSlice = (
  stores: PreferenceSliceDeps,
  options: PreferenceSliceOptions,
) => {
  const {
    showHidden,
    hiddenFilesLast,
    highContrast,
    foldersFirst,
    startDirPref,
    confirmDelete,
    sortField,
    sortDirection,
    sortFieldPref,
    sortDirectionPref,
    archiveName,
    archiveLevel,
    openDestAfterExtract,
    videoThumbs,
    hardwareAcceleration,
    ffmpegPath,
    thumbCacheMb,
    mountsPollMs,
    doubleClickMs,
    density,
  } = stores
  const { clearFacetCache, refreshForSort } = options

  const setSortFieldPref = async (field: DefaultSortField) => {
    if (get(sortField) === field) return
    sortField.set(field)
    sortFieldPref.set(field)
    void storeSortField(field)
    await refreshForSort()
  }

  const setSortDirectionPref = async (dir: SortDirection) => {
    if (get(sortDirection) === dir) return
    sortDirection.set(dir)
    sortDirectionPref.set(dir)
    void storeSortDirection(dir)
    await refreshForSort()
  }

  const setArchiveNamePref = (value: string) => {
    const trimmed = value.trim().replace(/\.zip$/i, '')
    if (trimmed.length === 0) {
      // Allow empty in UI; keep last persisted value until user provides one.
      archiveName.set('')
      return
    }
    archiveName.set(trimmed)
    void storeArchiveName(trimmed)
  }

  const setArchiveLevelPref = (value: number) => {
    const lvl = Math.min(Math.max(Math.round(value), 0), 9)
    archiveLevel.set(lvl)
    void storeArchiveLevel(lvl)
  }

  const toggleShowHidden = () => {
    showHidden.update((v) => {
      const next = !v
      void storeShowHidden(next)
      return next
    })
    clearFacetCache()
  }

  const toggleHiddenFilesLast = () => {
    hiddenFilesLast.update((v) => {
      const next = !v
      void storeHiddenFilesLast(next)
      return next
    })
  }

  const toggleHighContrast = () => {
    highContrast.update((v) => {
      const next = !v
      void storeHighContrast(next)
      return next
    })
  }

  const toggleFoldersFirst = () => {
    foldersFirst.update((v) => {
      const next = !v
      void storeFoldersFirst(next)
      return next
    })
  }

  const setStartDirPref = (value: string) => {
    const next = value.trim()
    startDirPref.set(next || null)
    void storeStartDir(next)
  }

  const loadShowHiddenPref = async () => {
    try {
      const saved = await loadShowHidden()
      if (typeof saved === 'boolean') {
        showHidden.set(saved)
      }
    } catch (err) {
      console.error('Failed to load showHidden setting', err)
    }
  }

  const loadHiddenFilesLastPref = async () => {
    try {
      const saved = await loadHiddenFilesLast()
      if (typeof saved === 'boolean') {
        hiddenFilesLast.set(saved)
      }
    } catch (err) {
      console.error('Failed to load hiddenFilesLast setting', err)
    }
  }

  const loadHighContrastPref = async () => {
    try {
      const saved = await loadHighContrast()
      if (typeof saved === 'boolean') {
        highContrast.set(saved)
      }
    } catch (err) {
      console.error('Failed to load highContrast setting', err)
    }
  }

  const loadStartDirPref = async () => {
    try {
      const saved = await loadStartDir()
      if (typeof saved === 'string' && saved.trim().length > 0) {
        startDirPref.set(saved)
      }
    } catch (err) {
      console.error('Failed to load startDir setting', err)
    }
  }

  const loadConfirmDeletePref = async () => {
    try {
      const saved = await loadConfirmDelete()
      if (typeof saved === 'boolean') {
        confirmDelete.set(saved)
      }
    } catch (err) {
      console.error('Failed to load confirmDelete setting', err)
    }
  }

  const toggleConfirmDelete = () => {
    confirmDelete.update((v) => {
      const next = !v
      void storeConfirmDelete(next)
      return next
    })
  }

  const loadSortPref = async () => {
    try {
      const savedField = await loadSortField()
      if (savedField === 'name' || savedField === 'type' || savedField === 'modified' || savedField === 'size') {
        sortField.set(savedField)
        sortFieldPref.set(savedField)
      } else if (savedField !== null) {
        await storeSortField('name')
        sortField.set('name')
        sortFieldPref.set('name')
      }
      const savedDir = await loadSortDirection()
      if (savedDir === 'asc' || savedDir === 'desc') {
        sortDirection.set(savedDir)
        sortDirectionPref.set(savedDir)
      }
    } catch (err) {
      console.error('Failed to load sort settings', err)
    }
  }

  const loadArchiveNamePref = async () => {
    try {
      const saved = await loadArchiveName()
      if (typeof saved === 'string' && saved.trim().length > 0) {
        archiveName.set(saved.trim())
      }
    } catch (err) {
      console.error('Failed to load archiveName setting', err)
    }
  }

  const loadArchiveLevelPref = async () => {
    try {
      const saved = await loadArchiveLevel()
      if (typeof saved === 'number' && saved >= 0 && saved <= 9) {
        archiveLevel.set(saved)
      }
    } catch (err) {
      console.error('Failed to load archiveLevel setting', err)
    }
  }

  const loadOpenDestAfterExtractPref = async () => {
    try {
      const saved = await loadOpenDestAfterExtract()
      if (typeof saved === 'boolean') {
        openDestAfterExtract.set(saved)
      }
    } catch (err) {
      console.error('Failed to load openDestAfterExtract setting', err)
    }
  }

  const loadVideoThumbsPref = async () => {
    try {
      const saved = await loadVideoThumbs()
      if (typeof saved === 'boolean') {
        videoThumbs.set(saved)
      }
    } catch (err) {
      console.error('Failed to load videoThumbs setting', err)
    }
  }

  const loadHardwareAccelerationPref = async () => {
    try {
      const saved = await loadHardwareAcceleration()
      if (typeof saved === 'boolean') {
        hardwareAcceleration.set(saved)
      }
    } catch (err) {
      console.error('Failed to load hardwareAcceleration setting', err)
    }
  }

  const loadFfmpegPathPref = async () => {
    try {
      const saved = await loadFfmpegPath()
      if (typeof saved === 'string') {
        ffmpegPath.set(saved)
      }
    } catch (err) {
      console.error('Failed to load ffmpegPath setting', err)
    }
  }

  const loadThumbCachePref = async () => {
    try {
      const saved = await loadThumbCacheMb()
      if (typeof saved === 'number' && saved >= 50 && saved <= 1000) {
        thumbCacheMb.set(saved)
      }
    } catch (err) {
      console.error('Failed to load thumbCacheMb setting', err)
    }
  }

  const loadDensityPref = async () => {
    try {
      const saved = await loadDensity()
      if (saved === 'cozy' || saved === 'compact') {
        density.set(saved)
      }
    } catch (err) {
      console.error('Failed to load density setting', err)
    }
  }

  const setDensityPref = (value: Density) => {
    density.set(value)
    void storeDensity(value)
  }

  const toggleOpenDestAfterExtract = () => {
    openDestAfterExtract.update((v) => {
      const next = !v
      void storeOpenDestAfterExtract(next)
      return next
    })
  }

  const toggleVideoThumbs = () => {
    videoThumbs.update((v) => {
      const next = !v
      void storeVideoThumbs(next)
      return next
    })
  }

  const setHardwareAccelerationPref = (value: boolean) => {
    hardwareAcceleration.set(value)
    void storeHardwareAcceleration(value)
  }

  const setFfmpegPathPref = (value: string) => {
    const normalized = value.trim()
    ffmpegPath.set(normalized)
    void storeFfmpegPath(normalized)
  }

  const setThumbCachePref = (value: number) => {
    const clamped = Math.min(1000, Math.max(50, Math.round(value)))
    thumbCacheMb.set(clamped)
    void storeThumbCacheMb(clamped)
  }

  const setMountsPollPref = (value: number) => {
    const clamped = Math.min(10000, Math.max(500, Math.round(value)))
    mountsPollMs.set(clamped)
    void storeMountsPollMs(clamped)
  }

  const setDoubleClickMsPref = (value: number) => {
    const clamped = Math.min(600, Math.max(150, Math.round(value)))
    doubleClickMs.set(clamped)
    void storeDoubleClickMs(clamped)
  }

  const loadMountsPollPref = async () => {
    try {
      const saved = await loadMountsPollMs()
      if (typeof saved === 'number') {
        const clamped = Math.min(10000, Math.max(500, Math.round(saved)))
        mountsPollMs.set(clamped)
      }
    } catch (err) {
      console.error('Failed to load mounts poll setting', err)
    }
  }

  const loadDoubleClickMsPref = async () => {
    try {
      const saved = await loadDoubleClickMs()
      if (typeof saved === 'number') {
        const clamped = Math.min(600, Math.max(150, Math.round(saved)))
        doubleClickMs.set(clamped)
      }
    } catch (err) {
      console.error('Failed to load doubleClickMs setting', err)
    }
  }

  const loadFoldersFirstPref = async () => {
    try {
      const saved = await loadFoldersFirst()
      if (typeof saved === 'boolean') {
        foldersFirst.set(saved)
      }
    } catch (err) {
      console.error('Failed to load foldersFirst setting', err)
    }
  }

  return {
    setSortFieldPref,
    setSortDirectionPref,
    setArchiveNamePref,
    setArchiveLevelPref,
    toggleShowHidden,
    toggleHiddenFilesLast,
    toggleHighContrast,
    toggleFoldersFirst,
    setStartDirPref,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadHighContrastPref,
    loadStartDirPref,
    loadConfirmDeletePref,
    toggleConfirmDelete,
    loadSortPref,
    loadArchiveNamePref,
    loadArchiveLevelPref,
    loadOpenDestAfterExtractPref,
    loadVideoThumbsPref,
    loadHardwareAccelerationPref,
    loadFfmpegPathPref,
    loadThumbCachePref,
    loadDensityPref,
    setDensityPref,
    toggleOpenDestAfterExtract,
    toggleVideoThumbs,
    setHardwareAccelerationPref,
    setFfmpegPathPref,
    setThumbCachePref,
    setMountsPollPref,
    setDoubleClickMsPref,
    loadMountsPollPref,
    loadDoubleClickMsPref,
    loadFoldersFirstPref,
  }
}
