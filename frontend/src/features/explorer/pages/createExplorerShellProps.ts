import type { OpenWithChoice } from '../services/openWith.service'
import type { SortField } from '../model/types'
import type { AdvancedRenamePayload } from '../modals/advancedRenameModal'

type AnyFn = (...args: any[]) => any

type Params = {
  sidebarCollapsed: boolean
  places: any
  bookmarks: any
  partitions: any
  handlePlace: AnyFn
  handleSidebarBookmarkSelect: AnyFn
  handleSidebarRemoveBookmark: AnyFn
  handleBookmarkDragOver: AnyFn
  handleBookmarkDragLeave: AnyFn
  handleBookmarkDrop: AnyFn
  handleSidebarPartitionSelect: AnyFn
  handleSidebarPartitionEject: AnyFn

  mode: 'address' | 'filter'
  isSearchSessionEnabled: boolean
  loading: boolean
  viewMode: 'list' | 'grid'
  showHidden: boolean
  activity: any
  handleInputFocus: AnyFn
  handleInputBlur: AnyFn
  submitPath: AnyFn
  submitSearch: AnyFn
  transitionToAddressMode: AnyFn
  currentPathValue: string
  navigateToBreadcrumb: (path: string) => void | Promise<void>
  handleTopbarAction: AnyFn
  handleTopbarViewModeChange: AnyFn

  errorMessage: string
  searchRunning: boolean
  filterActive: boolean
  filterValue: string
  cols: any
  gridTemplate: string
  filterSourceEntries: any
  filteredEntries: any
  visibleEntries: any
  columnFilters: any
  columnFacets: any
  columnFacetsLoading: boolean
  ensureColumnFacets: AnyFn
  start: number
  offsetY: number
  totalHeight: number
  selected: Set<string>
  sortField: SortField
  sortDirection: 'asc' | 'desc'
  isHidden: AnyFn
  displayName: AnyFn
  formatSize: AnyFn
  formatItems: AnyFn
  clipboardMode: 'copy' | 'cut'
  clipboardPaths: Set<string>
  handleRowsScrollCombined: AnyFn
  handleWheelCombined: AnyFn
  handleRowsKeydownCombined: AnyFn
  handleRowsMouseDown: AnyFn
  handleRowsClickSafe: AnyFn
  handleBlankContextMenu: AnyFn
  changeSort: AnyFn
  toggleColumnFilter: AnyFn
  resetColumnFilter: AnyFn
  startResize: AnyFn
  ariaSort: AnyFn
  handleRowClickWithOpen: AnyFn
  handleOpenEntry: AnyFn
  handleRowContextMenu: AnyFn
  toggleStar: AnyFn
  handleRowDragStart: AnyFn
  handleRowDragEnd: AnyFn
  handleRowDragEnter: AnyFn
  handleRowDragOver: AnyFn
  handleRowDrop: AnyFn
  handleRowDragLeave: AnyFn
  dragTargetPath: string | null
  dragPathsLength: number
  dragging: boolean
  handleBreadcrumbDragOver: AnyFn
  handleBreadcrumbDragLeave: AnyFn
  handleBreadcrumbDrop: AnyFn
  selectionActive: boolean
  selectionRect: any
  videoThumbs: boolean
  currentView: string
  thumbnailRefreshToken: number

  contextMenu: any
  blankMenu: any
  handleContextSelect: AnyFn
  handleBlankContextAction: AnyFn
  closeContextMenu: AnyFn
  closeBlankContextMenu: AnyFn

  deleteState: any
  deleteModal: any
  renameState: any
  confirmRename: AnyFn
  closeRenameModal: AnyFn
  advancedRenameState: any
  advancedRenameModal: any
  compressState: any
  confirmCompress: AnyFn
  closeCompress: AnyFn
  checkDuplicatesState: any
  checkDuplicatesModal: any
  copyCheckDuplicatesList: AnyFn
  searchCheckDuplicates: AnyFn
  closeCheckDuplicatesModal: AnyFn
  newFolderState: any
  confirmNewFolder: AnyFn
  closeNewFolderModal: AnyFn
  newFileState: any
  newFileTypeHint: string
  confirmNewFile: AnyFn
  closeNewFileModal: AnyFn
  openWithState: any
  openWithModal: any
  propertiesState: any
  propertiesModal: any
  bookmarkModalOpen: boolean
  bookmarkCandidate: any
  confirmBookmark: AnyFn
  closeBookmarkModal: AnyFn
  toastMessage: string | null

  selectionText: string
}

export const createExplorerShellProps = (p: Params) => ({
  sidebarProps: {
    collapsed: p.sidebarCollapsed,
    places: p.places,
    bookmarks: p.bookmarks,
    partitions: p.partitions,
    onPlaceSelect: p.handlePlace,
    onBookmarkSelect: p.handleSidebarBookmarkSelect,
    onRemoveBookmark: p.handleSidebarRemoveBookmark,
    dragTargetPath: p.dragTargetPath,
    onBookmarkDragOver: p.handleBookmarkDragOver,
    onBookmarkDragLeave: p.handleBookmarkDragLeave,
    onBookmarkDrop: p.handleBookmarkDrop,
    onPartitionSelect: p.handleSidebarPartitionSelect,
    onPartitionEject: p.handleSidebarPartitionEject,
  },

  topbarProps: {
    mode: p.mode,
    searchMode: p.isSearchSessionEnabled,
    loading: p.loading,
    viewMode: p.viewMode,
    showHidden: p.showHidden,
    activity: p.activity,
    onFocus: p.handleInputFocus,
    onBlur: p.handleInputBlur,
    onSubmitPath: p.submitPath,
    onSearch: p.submitSearch,
    onExitSearch: () => void p.transitionToAddressMode({ path: p.currentPathValue, blur: true }),
    onNavigateSegment: (path: string) => void p.navigateToBreadcrumb(path),
    onTopbarAction: p.handleTopbarAction,
    onTopbarViewModeChange: p.handleTopbarViewModeChange,
  },

  listingProps: {
    noticeMessage: p.errorMessage,
    searchRunning: p.searchRunning,
    filterActive: p.filterActive,
    filterValue: p.filterValue,
    currentPath: p.currentPathValue,
    cols: p.cols,
    gridTemplate: p.gridTemplate,
    filterSourceEntries: p.filterSourceEntries,
    filteredEntries: p.filteredEntries,
    visibleEntries: p.visibleEntries,
    columnFilters: p.columnFilters,
    columnFacets: p.columnFacets,
    columnFacetsLoading: p.columnFacetsLoading,
    onEnsureColumnFacets: p.ensureColumnFacets,
    start: p.start,
    offsetY: p.offsetY,
    totalHeight: p.totalHeight,
    wide: p.sidebarCollapsed,
    selected: p.selected,
    sortField: p.sortField,
    sortDirection: p.sortDirection,
    isHidden: p.isHidden,
    displayName: p.displayName,
    formatSize: p.formatSize,
    formatItems: p.formatItems,
    clipboardMode: p.clipboardMode,
    clipboardPaths: p.clipboardPaths,
    onRowsScroll: p.handleRowsScrollCombined,
    onWheel: p.handleWheelCombined,
    onRowsKeydown: p.handleRowsKeydownCombined,
    onRowsMousedown: p.handleRowsMouseDown,
    onRowsClick: p.handleRowsClickSafe,
    onRowsContextMenu: p.handleBlankContextMenu,
    onChangeSort: p.changeSort,
    onToggleFilter: (field: SortField, id: string, checked: boolean) => p.toggleColumnFilter(field, id, checked),
    onResetFilter: (field: SortField) => p.resetColumnFilter(field),
    onStartResize: p.startResize,
    ariaSort: p.ariaSort,
    onRowClick: p.handleRowClickWithOpen,
    onOpen: p.handleOpenEntry,
    onContextMenu: p.handleRowContextMenu,
    onToggleStar: p.toggleStar,
    onRowDragStart: p.handleRowDragStart,
    onRowDragEnd: p.handleRowDragEnd,
    onRowDragEnter: p.handleRowDragEnter,
    onRowDragOver: p.handleRowDragOver,
    onRowDrop: p.handleRowDrop,
    onRowDragLeave: p.handleRowDragLeave,
    dragTargetPath: p.dragTargetPath,
    dragAllowed: p.dragPathsLength > 0,
    dragging: p.dragging,
    onBreadcrumbDragOver: p.handleBreadcrumbDragOver,
    onBreadcrumbDragLeave: p.handleBreadcrumbDragLeave,
    onBreadcrumbDrop: p.handleBreadcrumbDrop,
    selectionActive: p.selectionActive,
    selectionRect: p.selectionRect,
    videoThumbs: p.videoThumbs,
    thumbnailsEnabled: p.currentView !== 'trash',
    thumbnailRefreshToken: p.thumbnailRefreshToken,
  },

  menuProps: {
    contextMenu: p.contextMenu,
    blankMenu: p.blankMenu,
    onContextSelect: p.handleContextSelect,
    onBlankContextSelect: p.handleBlankContextAction,
    onCloseContextMenu: p.closeContextMenu,
    onCloseBlankContextMenu: p.closeBlankContextMenu,
  },

  modalProps: {
    deleteConfirmOpen: p.deleteState.open,
    deleteTargets: p.deleteState.targets,
    onConfirmDelete: p.deleteModal.confirm,
    onCancelDelete: p.deleteModal.close,
    renameModalOpen: p.renameState.open,
    renameTarget: p.renameState.target,
    renameError: p.renameState.error,
    onConfirmRename: p.confirmRename,
    onCancelRename: p.closeRenameModal,
    advancedRenameOpen: p.advancedRenameState.open,
    advancedRenameEntries: p.advancedRenameState.entries,
    advancedRenameRegex: p.advancedRenameState.regex,
    advancedRenameReplacement: p.advancedRenameState.replacement,
    advancedRenamePrefix: p.advancedRenameState.prefix,
    advancedRenameSuffix: p.advancedRenameState.suffix,
    advancedRenameCaseSensitive: p.advancedRenameState.caseSensitive,
    advancedRenameKeepExtension: p.advancedRenameState.keepExtension,
    advancedRenameSequenceMode: p.advancedRenameState.sequenceMode,
    advancedRenameSequencePlacement: p.advancedRenameState.sequencePlacement,
    advancedRenameSequenceStart: p.advancedRenameState.sequenceStart,
    advancedRenameSequenceStep: p.advancedRenameState.sequenceStep,
    advancedRenameSequencePad: p.advancedRenameState.sequencePad,
    advancedRenameError: p.advancedRenameState.error,
    advancedRenamePreview: p.advancedRenameState.preview,
    advancedRenamePreviewError: p.advancedRenameState.previewError,
    advancedRenamePreviewLoading: p.advancedRenameState.previewLoading,
    onAdvancedRenameChange: (payload: AdvancedRenamePayload) => p.advancedRenameModal.change(payload),
    onConfirmAdvancedRename: () => p.advancedRenameModal.confirm(),
    onCancelAdvancedRename: () => p.advancedRenameModal.close(),
    compressOpen: p.compressState.open,
    compressError: p.compressState.error,
    onConfirmCompress: p.confirmCompress,
    onCancelCompress: p.closeCompress,
    checkDuplicatesOpen: p.checkDuplicatesState.open,
    checkDuplicatesTarget: p.checkDuplicatesState.target,
    checkDuplicatesSearchRoot: p.checkDuplicatesState.searchRoot,
    checkDuplicatesDuplicates: p.checkDuplicatesState.duplicates,
    checkDuplicatesScanning: p.checkDuplicatesState.scanning,
    checkDuplicatesProgressPercent: p.checkDuplicatesState.progressPercent,
    checkDuplicatesProgressLabel: p.checkDuplicatesState.progressLabel,
    checkDuplicatesError: p.checkDuplicatesState.error,
    onChangeCheckDuplicatesSearchRoot: p.checkDuplicatesModal.setSearchRoot,
    onCopyCheckDuplicates: p.copyCheckDuplicatesList,
    onSearchCheckDuplicates: p.searchCheckDuplicates,
    onCloseCheckDuplicates: p.closeCheckDuplicatesModal,
    newFolderOpen: p.newFolderState.open,
    newFolderError: p.newFolderState.error,
    onConfirmNewFolder: p.confirmNewFolder,
    onCancelNewFolder: p.closeNewFolderModal,
    newFileOpen: p.newFileState.open,
    newFileError: p.newFileState.error,
    newFileTypeHint: p.newFileTypeHint,
    onConfirmNewFile: p.confirmNewFile,
    onCancelNewFile: p.closeNewFileModal,
    openWithOpen: p.openWithState.open,
    openWithApps: p.openWithState.apps,
    openWithLoading: p.openWithState.loading,
    openWithError: p.openWithState.error,
    openWithBusy: p.openWithState.submitting,
    onConfirmOpenWith: (choice: OpenWithChoice) => p.openWithModal.confirm(choice),
    onCloseOpenWith: p.openWithModal.close,
    propertiesOpen: p.propertiesState.open,
    propertiesEntry: p.propertiesState.entry,
    propertiesMutationsLocked: p.propertiesState.mutationsLocked,
    propertiesCount: p.propertiesState.count,
    propertiesSize: p.propertiesState.size,
    propertiesItemCount: p.propertiesState.itemCount,
    propertiesHidden: p.propertiesState.hidden,
    propertiesExtraMetadataLoading: p.propertiesState.extraMetadataLoading,
    propertiesExtraMetadataError: p.propertiesState.extraMetadataError,
    propertiesExtraMetadata: p.propertiesState.extraMetadata,
    propertiesPermissionsLoading: p.propertiesState.permissionsLoading,
    propertiesPermissionsApplying: p.propertiesState.permissionsApplying,
    propertiesOwnershipApplying: p.propertiesState.ownershipApplying,
    propertiesOwnershipError: p.propertiesState.ownershipError,
    propertiesOwnershipUsers: p.propertiesState.ownershipUsers,
    propertiesOwnershipGroups: p.propertiesState.ownershipGroups,
    propertiesOwnershipOptionsLoading: p.propertiesState.ownershipOptionsLoading,
    propertiesOwnershipOptionsError: p.propertiesState.ownershipOptionsError,
    propertiesPermissions: p.propertiesState.permissions,
    onTogglePermissionsAccess: (
      scope: 'owner' | 'group' | 'other',
      key: 'read' | 'write' | 'exec',
      next: boolean,
    ) => p.propertiesModal.toggleAccess(scope, key, next),
    onSetOwnership: (owner: string, group: string) => p.propertiesModal.setOwnership(owner, group),
    onToggleHidden: (next: boolean) => p.propertiesModal.toggleHidden(next),
    onLoadPropertiesExtraMetadata: () => p.propertiesModal.loadExtraIfNeeded(),
    onCloseProperties: p.propertiesModal.close,
    bookmarkModalOpen: p.bookmarkModalOpen,
    bookmarkCandidate: p.bookmarkCandidate,
    onConfirmBookmark: p.confirmBookmark,
    onCancelBookmark: p.closeBookmarkModal,
    toastMessage: p.toastMessage,
  },

  statusProps: {
    selectionText: p.selectionText,
  },
})
