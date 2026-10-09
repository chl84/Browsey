import type { ComponentProps } from 'svelte'
import type { Readable } from 'svelte/store'
import type ExplorerShell from '../ui-shell/components/ExplorerShell.svelte'
import type { useModalsController } from '../hooks/useModalsController'
import type { useExplorerSearchSession } from '../navigation/useExplorerSearchSession'
import type { CurrentView } from '../context/createContextActions'
import type { OpenWithChoice } from '../services/openWith.service'
import type { SortField } from '../model/types'
import type { AdvancedRenamePayload } from '../modals/advancedRenameModal'

// Use the consuming component's contract rather than a parallel prop schema.
type ShellProps = ComponentProps<typeof ExplorerShell>
export type ExplorerShellProps = Pick<ShellProps,
  'sidebarProps' | 'topbarProps' | 'listingProps' | 'menuProps' | 'modalProps' | 'statusProps'>
type SidebarProps = ExplorerShellProps['sidebarProps']
type TopbarProps = ExplorerShellProps['topbarProps']
type ListingProps = ExplorerShellProps['listingProps']
type MenuProps = ExplorerShellProps['menuProps']
type ModalProps = ExplorerShellProps['modalProps']
type Modals = ReturnType<typeof useModalsController>
type StoreValue<T> = T extends Readable<infer Value> ? Value : never

export type ExplorerShellParams = {
  sidebarCollapsed: boolean
  places: SidebarProps['places']
  bookmarks: SidebarProps['bookmarks']
  partitions: SidebarProps['partitions']
  handlePlace: SidebarProps['onPlaceSelect']
  handleEmptyWastebasket: SidebarProps['onEmptyWastebasket']
  handleSidebarBookmarkSelect: SidebarProps['onBookmarkSelect']
  handleSidebarRemoveBookmark: SidebarProps['onRemoveBookmark']
  handleBookmarkDragOver: SidebarProps['onBookmarkDragOver']
  handleBookmarkDragLeave: SidebarProps['onBookmarkDragLeave']
  handleBookmarkDrop: SidebarProps['onBookmarkDrop']
  handleSidebarPartitionSelect: SidebarProps['onPartitionSelect']
  handleSidebarPartitionEject: SidebarProps['onPartitionEject']
  handleSidebarPartitionFormat: SidebarProps['onPartitionFormat']

  mode: 'address' | 'filter'
  isSearchSessionEnabled: boolean
  loading: boolean
  viewMode: 'list' | 'grid'
  showHidden: boolean
  activity: TopbarProps['activity']
  handleInputFocus: TopbarProps['onFocus']
  handleInputBlur: TopbarProps['onBlur']
  goBack: TopbarProps['onGoBack']
  goForward: TopbarProps['onGoForward']
  submitPath: TopbarProps['onSubmitPath']
  submitSearch: TopbarProps['onSearch']
  transitionToAddressMode: ReturnType<typeof useExplorerSearchSession>['transitionToAddressMode']
  currentPathValue: string
  navigateToBreadcrumb: (path: string) => void | Promise<void>
  handleTopbarAction: TopbarProps['onTopbarAction']
  handleTopbarViewModeChange: TopbarProps['onTopbarViewModeChange']

  errorMessage: string
  searchRunning: boolean
  filterActive: boolean
  filterValue: string
  cols: ListingProps['cols']
  gridTemplate: string
  filterSourceEntries: ListingProps['filterSourceEntries']
  filteredEntries: ListingProps['filteredEntries']
  visibleEntries: ListingProps['visibleEntries']
  columnFilters: ListingProps['columnFilters']
  columnFacets: ListingProps['columnFacets']
  columnFacetsLoading: boolean
  ensureColumnFacets: ListingProps['onEnsureColumnFacets']
  start: number
  offsetY: number
  totalHeight: number
  selected: Set<string>
  sortField: SortField
  sortDirection: 'asc' | 'desc'
  isHidden: ListingProps['isHidden']
  displayName: ListingProps['displayName']
  formatSize: ListingProps['formatSize']
  formatItems: ListingProps['formatItems']
  clipboardMode: 'copy' | 'cut'
  clipboardPaths: Set<string>
  handleRowsScrollCombined: ListingProps['onRowsScroll']
  handleWheelCombined: ListingProps['onWheel']
  handleRowsKeydownCombined: ListingProps['onRowsKeydown']
  handleRowsMouseDown: ListingProps['onRowsMousedown']
  handleRowsClickSafe: ListingProps['onRowsClick']
  handleBlankContextMenu: ListingProps['onRowsContextMenu']
  changeSort: ListingProps['onChangeSort']
  toggleColumnFilter: ListingProps['onToggleFilter']
  resetColumnFilter: ListingProps['onResetFilter']
  startResize: ListingProps['onStartResize']
  ariaSort: ListingProps['ariaSort']
  handleRowClickWithOpen: ListingProps['onRowClick']
  handleOpenEntry: ListingProps['onOpen']
  handleRowContextMenu: ListingProps['onContextMenu']
  toggleStar: ListingProps['onToggleStar']
  handleRowDragStart: ListingProps['onRowDragStart']
  handleRowDragEnd: ListingProps['onRowDragEnd']
  handleRowDragEnter: ListingProps['onRowDragEnter']
  handleRowDragOver: ListingProps['onRowDragOver']
  handleRowDrop: ListingProps['onRowDrop']
  handleRowDragLeave: ListingProps['onRowDragLeave']
  dragTargetPath: string | null
  dragPathsLength: number
  dragging: boolean
  handleBreadcrumbDragOver: ListingProps['onBreadcrumbDragOver']
  handleBreadcrumbDragLeave: ListingProps['onBreadcrumbDragLeave']
  handleBreadcrumbDrop: ListingProps['onBreadcrumbDrop']
  selectionActive: boolean
  selectionRect: ListingProps['selectionRect']
  videoThumbs: boolean
  cloudThumbs: boolean
  currentView: CurrentView
  thumbnailRefreshToken: number
  gridThumbSize: number
  gridCardWidth: number
  gridRowHeight: number

  contextMenu: MenuProps['contextMenu']
  blankMenu: MenuProps['blankMenu']
  handleContextSelect: MenuProps['onContextSelect']
  handleBlankContextAction: MenuProps['onBlankContextSelect']
  closeContextMenu: MenuProps['onCloseContextMenu']
  closeBlankContextMenu: MenuProps['onCloseBlankContextMenu']

  deleteState: StoreValue<Modals['deleteState']>
  deleteModal: Modals['deleteModal']
  renameState: StoreValue<Modals['renameState']>
  confirmRename: ModalProps['onConfirmRename']
  closeRenameModal: ModalProps['onCancelRename']
  advancedRenameState: StoreValue<Modals['advancedRenameState']>
  advancedRenameModal: Modals['advancedRenameModal']
  compressState: StoreValue<Modals['compressState']>
  confirmCompress: ModalProps['onConfirmCompress']
  closeCompress: ModalProps['onCancelCompress']
  checkDuplicatesState: StoreValue<Modals['checkDuplicatesState']>
  checkDuplicatesModal: Modals['checkDuplicatesModal']
  copyCheckDuplicatesList: ModalProps['onCopyCheckDuplicates']
  searchCheckDuplicates: ModalProps['onSearchCheckDuplicates']
  closeCheckDuplicatesModal: ModalProps['onCloseCheckDuplicates']
  newFolderState: StoreValue<Modals['newFolderState']>
  confirmNewFolder: ModalProps['onConfirmNewFolder']
  closeNewFolderModal: ModalProps['onCancelNewFolder']
  newFileState: StoreValue<Modals['newFileState']>
  newFileTypeHint: string
  confirmNewFile: ModalProps['onConfirmNewFile']
  closeNewFileModal: ModalProps['onCancelNewFile']
  openWithState: StoreValue<Modals['openWithState']>
  openWithModal: Modals['openWithModal']
  propertiesState: StoreValue<Modals['propertiesState']>
  propertiesModal: Modals['propertiesModal']
  bookmarkModalOpen: boolean
  bookmarkCandidate: ModalProps['bookmarkCandidate']
  confirmBookmark: ModalProps['onConfirmBookmark']
  closeBookmarkModal: ModalProps['onCancelBookmark']
  toastMessage: string | null

  selectionText: string
}

export const createExplorerShellProps = (p: ExplorerShellParams): ExplorerShellProps => ({
  sidebarProps: {
    collapsed: p.sidebarCollapsed,
    places: p.places,
    bookmarks: p.bookmarks,
    partitions: p.partitions,
    onPlaceSelect: p.handlePlace,
    onEmptyWastebasket: p.handleEmptyWastebasket,
    onBookmarkSelect: p.handleSidebarBookmarkSelect,
    onRemoveBookmark: p.handleSidebarRemoveBookmark,
    dragTargetPath: p.dragTargetPath,
    onBookmarkDragOver: p.handleBookmarkDragOver,
    onBookmarkDragLeave: p.handleBookmarkDragLeave,
    onBookmarkDrop: p.handleBookmarkDrop,
    onPartitionSelect: p.handleSidebarPartitionSelect,
    onPartitionEject: p.handleSidebarPartitionEject,
    onPartitionFormat: p.handleSidebarPartitionFormat,
    onPartitionProperties: p.propertiesModal.openPartition,
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
    onGoBack: () => void p.goBack(),
    onGoForward: () => void p.goForward(),
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
    cloudThumbs: p.cloudThumbs,
    thumbnailsEnabled: p.currentView !== 'trash',
    thumbnailRefreshToken: p.thumbnailRefreshToken,
    gridThumbSize: p.gridThumbSize,
    gridCardWidth: p.gridCardWidth,
    gridRowHeight: p.gridRowHeight,
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
    deleteMode: p.deleteState.mode,
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
    openWithProgress: p.openWithState.progress,
    openWithError: p.openWithState.error,
    openWithBusy: p.openWithState.submitting,
    onConfirmOpenWith: (choice: OpenWithChoice) => p.openWithModal.confirm(choice),
    onCloseOpenWith: p.openWithModal.close,
    propertiesOpen: p.propertiesState.open,
    propertiesEntry: p.propertiesState.entry,
    propertiesPartition: p.propertiesState.partition,
    propertiesVolumeUsage: p.propertiesState.volumeUsage,
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
    onCopyParentFolder: () => p.propertiesModal.copyParentFolder(),
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
