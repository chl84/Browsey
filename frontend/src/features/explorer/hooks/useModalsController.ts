import { createDeleteConfirmModal } from '../modals/deleteConfirmModal'
import { createOpenWithModal } from '../modals/openWithModal'
import { createPropertiesModal } from '../modals/propertiesModal'
import { createRenameModal } from '../modals/renameModal'
import { createNewFolderModal } from '../modals/newFolderModal'
import { createCompressModal } from '../modals/compressModal'
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

  const propertiesModal = createPropertiesModal({ computeDirStats })
  const propertiesState = propertiesModal.state

  const renameModal = createRenameModal({
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    parentPath,
  })
  const renameState = renameModal.state

  const newFolderModal = createNewFolderModal({
    getCurrentPath,
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    showToast,
  })
  const newFolderState = newFolderModal.state

  const compressModal = createCompressModal({
    activityApi,
    getCurrentPath,
    loadPath: (path: string) => loadPath(path, { recordHistory: false }),
    showToast,
  })
  const compressState = compressModal.state

  const actions = {
    openWith: (entry: Entry) => openWithModal.open(entry),
    openCompress: (entries: Entry[]) => compressModal.open(entries),
    startRename: (entry: Entry) => renameModal.open(entry),
    confirmDelete: (entries: Entry[]) => deleteModal.open(entries),
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
    newFolderModal,
    newFolderState,
    compressModal,
    compressState,
    actions,
  }
}
