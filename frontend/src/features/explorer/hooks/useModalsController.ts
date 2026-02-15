import { createDeleteConfirmModal } from '../modals/deleteConfirmModal'
import { createOpenWithModal } from '../modals/openWithModal'
import { createPropertiesModal } from '../modals/propertiesModal'
import { createRenameModal } from '../modals/renameModal'
import { createAdvancedRenameModal } from '../modals/advancedRenameModal'
import { createNewFolderModal } from '../modals/newFolderModal'
import { createNewFileModal } from '../modals/newFileModal'
import { createCompressModal } from '../modals/compressModal'
import { createCheckDuplicatesModal } from '../modals/checkDuplicatesModal'
import type { Entry } from '../types'

type Deps = {
  activityApi: ReturnType<typeof import('./useActivity').createActivity>
  reloadCurrent: () => Promise<void>
  showToast: (msg: string, timeout?: number) => void
  getCurrentPath: () => string
  loadPath: (path: string, opts?: { recordHistory?: boolean; silent?: boolean }) => Promise<void>
  parentPath: (path: string) => string
  computeDirStats: (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ) => Promise<{ total: number; items: number }>
}

export const useModalsController = ({
  activityApi,
  reloadCurrent,
  showToast,
  getCurrentPath,
  loadPath,
  parentPath,
  computeDirStats,
}: Deps) => {
  const deleteModal = createDeleteConfirmModal({ activityApi, reloadCurrent, showToast })
  const deleteState = deleteModal.state

  const openWithModal = createOpenWithModal({ showToast })
  const openWithState = openWithModal.state

  const propertiesModal = createPropertiesModal({ computeDirStats, showToast })
  const propertiesState = propertiesModal.state

  const renameModal = createRenameModal({
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    parentPath,
  })
  const renameState = renameModal.state

  const advancedRenameModal = createAdvancedRenameModal({
    reloadCurrent,
    showToast,
  })
  const advancedRenameState = advancedRenameModal.state

  const newFolderModal = createNewFolderModal({
    getCurrentPath,
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    showToast,
  })
  const newFolderState = newFolderModal.state

  const newFileModal = createNewFileModal({
    getCurrentPath,
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    showToast,
  })
  const newFileState = newFileModal.state

  const compressModal = createCompressModal({
    activityApi,
    getCurrentPath,
    reloadCurrent,
    showToast,
  })
  const compressState = compressModal.state

  const checkDuplicatesModal = createCheckDuplicatesModal({ parentPath })
  const checkDuplicatesState = checkDuplicatesModal.state

  const actions = {
    openWith: (entry: Entry) => openWithModal.open(entry),
    openCompress: (entries: Entry[], defaultArchiveBase: string) => compressModal.open(entries, defaultArchiveBase),
    openCheckDuplicates: (entry: Entry) => checkDuplicatesModal.open(entry),
    startRename: (entry: Entry) => renameModal.open(entry),
    startAdvancedRename: (entries: Entry[]) => advancedRenameModal.open(entries),
    confirmDelete: (entries: Entry[], mode: 'default' | 'trash' = 'default') => deleteModal.open(entries, mode),
    openProperties: (entries: Entry[]) => propertiesModal.open(entries),
  }

  return {
    deleteModal,
    deleteState,
    openWithModal,
    openWithState,
    propertiesModal,
    propertiesState,
    renameModal,
    renameState,
    advancedRenameModal,
    advancedRenameState,
    newFileModal,
    newFileState,
    newFolderModal,
    newFolderState,
    compressModal,
    compressState,
    checkDuplicatesModal,
    checkDuplicatesState,
    actions,
  }
}
