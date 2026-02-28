<script lang="ts">
import Sidebar from './Sidebar.svelte'
import Topbar from './Topbar.svelte'
import Notice from '../../../../shared/ui/Notice.svelte'
import FileList from '../../components/FileList.svelte'
import FileGrid from '../../components/FileGrid.svelte'
import Statusbar from '../../../../shared/ui/Statusbar.svelte'
import ContextMenu from '../../components/ContextMenu.svelte'
import DeleteConfirmModal from '../../components/DeleteConfirmModal.svelte'
import RenameModal from '../../components/RenameModal.svelte'
import NewFolderModal from '../../components/NewFolderModal.svelte'
import OpenWithModal from '../../components/OpenWithModal.svelte'
import PropertiesModal from '../../components/PropertiesModal.svelte'
import AdvancedRenameModal from '../../components/AdvancedRenameModal.svelte'
import BookmarkModal from '../../components/BookmarkModal.svelte'
import Toast from '../../../../shared/ui/Toast.svelte'
import CompressModal from '../../components/CompressModal.svelte'
import CheckDuplicatesModal from '../../components/CheckDuplicatesModal.svelte'
import type { Column, Entry, ListingFacets, Partition, SortField } from '../../model/types'
import type { ContextAction } from '../../context/createContextMenus'
import type { OpenWithApp, OpenWithChoice } from '../../services/openWith.service'
import type { AdvancedRenamePayload } from '../../modals/advancedRenameModal'

  let sidebarCollapsed = false
  let places: { label: string; path: string }[] = []
  let bookmarks: { label: string; path: string }[] = []
  let partitions: Partition[] = []
  let onPlaceSelect: (label: string, path: string) => void = () => {}
  let onBookmarkSelect: (path: string) => void = () => {}
  let onRemoveBookmark: (path: string) => void = () => {}
  let onBookmarkDragOver: (path: string, e: DragEvent) => void = () => {}
  let onBookmarkDragLeave: (path: string, e: DragEvent) => void = () => {}
  let onBookmarkDrop: (path: string, e: DragEvent) => void = () => {}
  let onPartitionSelect: (path: string) => void = () => {}
  let onPartitionEject: (path: string) => void = () => {}
  export let pathInput = ''
  export let pathInputEl: HTMLInputElement | null = null
  let mode: 'address' | 'filter' = 'address'
  let searchMode = false
  let loading = false
  let viewMode: 'list' | 'grid' = 'list'
  let onFocus: () => void = () => {}
  let onBlur: () => void = () => {}
  let onSubmitPath: () => void = () => {}
  let onSearch: () => void = () => {}
  let onExitSearch: () => void = () => {}
  let onNavigateSegment: (path: string) => void = () => {}
  let onTopbarAction: (
    id: 'open-settings' | 'open-shortcuts' | 'search' | 'toggle-hidden' | 'refresh' | 'about'
  ) => void = () => {}
  let onTopbarViewModeChange: (mode: 'list' | 'grid') => void = () => {}

  let noticeMessage = ''
  let searchRunning = false
  let filterActive = false
  let filterValue = ''
  let currentPath = ''

  let cols: Column[] = []
  let gridTemplate = ''
  export let rowsEl: HTMLDivElement | null = null
  export let headerEl: HTMLDivElement | null = null
  let filterSourceEntries: Entry[] = []
  let filteredEntries: Entry[] = []
  let visibleEntries: Entry[] = []
  let showHidden = false
  let columnFilters: {
    name: Set<string>
    type: Set<string>
    modified: Set<string>
    size: Set<string>
  } = { name: new Set(), type: new Set(), modified: new Set(), size: new Set() }
  let columnFacets: ListingFacets = { name: [], type: [], modified: [], size: [] }
  let columnFacetsLoading = false
  let onEnsureColumnFacets: () => void | Promise<void> = () => {}
  let start = 0
  let offsetY = 0
  let totalHeight = 0
  let wide = false
  let selected: Set<string> = new Set()
  let sortField: SortField = 'name'
  let sortDirection: 'asc' | 'desc' = 'asc'
  let isHidden: (entry: Entry) => boolean = () => false
  let displayName: (entry: Entry) => string = (e) => e.name
  let formatSize: (n: number | null | undefined) => string = (n) => String(n ?? '')
  let formatItems: (n?: number | null) => string = (n) => String(n ?? '')
  let clipboardMode: 'copy' | 'cut' = 'copy'
  let clipboardPaths: Set<string> = new Set()
  let onRowsScroll: (e: Event) => void = () => {}
  let onWheel: (e: WheelEvent) => void = () => {}
  let onRowsKeydown: (e: KeyboardEvent) => void = () => {}
  let onRowsMousedown: (e: MouseEvent) => void = () => {}
  let onRowsClick: (e: MouseEvent) => void = () => {}
  let onRowsContextMenu: (e: MouseEvent) => void = () => {}
  let onChangeSort: (field: SortField) => void = () => {}
  let onToggleFilter: (field: SortField, id: string, checked: boolean) => void = () => {}
  let onResetFilter: (field: SortField) => void = () => {}
  let onStartResize: (col: number, event: PointerEvent) => void = () => {}
  let ariaSort: (field: SortField) => 'ascending' | 'descending' | 'none' = () => 'none'
  let onRowClick: (entry: Entry, absoluteIndex: number, event: MouseEvent) => void = () => {}
  let onOpen: (entry: Entry) => void = () => {}
  let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
  let onToggleStar: (entry: Entry) => void = () => {}
  let onRowDragStart: (entry: Entry, event: DragEvent) => void = () => {}
  let onRowDragEnd: (event: DragEvent) => void = () => {}
  let onRowDragOver: (entry: Entry, event: DragEvent) => void = () => {}
  let onRowDragEnter: (entry: Entry, event: DragEvent) => void = () => {}
  let onRowDrop: (entry: Entry, event: DragEvent) => void = () => {}
  let onRowDragLeave: (entry: Entry, event: DragEvent) => void = () => {}

  let selectionText = ''
  let activity:
    | { label: string; percent: number | null; cancel?: (() => void) | null; cancelling?: boolean }
    | null = null
  let selectionActive = false
  let selectionRect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
  let dragTargetPath: string | null = null
  let dragAllowed = false
  let dragging = false
  let videoThumbs = true
  let thumbnailsEnabled = true
  let thumbnailRefreshToken = 0

  let contextMenu: { open: boolean; x: number; y: number; actions: ContextAction[] } = {
    open: false,
    x: 0,
    y: 0,
    actions: [],
  }
  let blankMenu: { open: boolean; x: number; y: number; actions: ContextAction[] } = {
    open: false,
    x: 0,
    y: 0,
    actions: [],
  }
  let onContextSelect: (id: string) => void = () => {}
  let onBlankContextSelect: (id: string) => void = () => {}
  let onCloseContextMenu: () => void = () => {}
  let onCloseBlankContextMenu: () => void = () => {}
  let onBreadcrumbDragOver: (path: string, e: DragEvent) => void = () => {}
  let onBreadcrumbDragLeave: (path: string, e: DragEvent) => void = () => {}
  let onBreadcrumbDrop: (path: string, e: DragEvent) => void = () => {}

  let deleteConfirmOpen = false
  let deleteTargets: Entry[] = []
  let onConfirmDelete: () => void = () => {}
  let onCancelDelete: () => void = () => {}

  let renameModalOpen = false
  let renameTarget: Entry | null = null
  export let renameValue = ''
  let renameError = ''
  let onConfirmRename: (name: string) => void = () => {}
  let onCancelRename: () => void = () => {}
  let advancedRenameOpen = false
  let advancedRenameEntries: Entry[] = []
  let advancedRenameRegex = ''
  let advancedRenameReplacement = ''
  let advancedRenamePrefix = ''
  let advancedRenameSuffix = ''
  let advancedRenameCaseSensitive = true
  let advancedRenameKeepExtension = true
  let advancedRenameSequenceMode: 'none' | 'numeric' | 'alpha' = 'none'
  let advancedRenameSequencePlacement: 'start' | 'end' = 'end'
  let advancedRenameSequenceStart = 1
  let advancedRenameSequenceStep = 1
  let advancedRenameSequencePad = 2
  let advancedRenameError = ''
  let advancedRenamePreview: { original: string; next: string }[] = []
  let advancedRenamePreviewError = ''
  let advancedRenamePreviewLoading = false
  let onAdvancedRenameChange: (payload: AdvancedRenamePayload) => void = () => {}
  let onConfirmAdvancedRename: () => void = () => {}
  let onCancelAdvancedRename: () => void = () => {}
  let compressOpen = false
  export let compressName = ''
  export let compressLevel = 6
  let compressError = ''
  let onConfirmCompress: (name: string, level: number) => void = () => {}
  let onCancelCompress: () => void = () => {}
  let checkDuplicatesOpen = false
  let checkDuplicatesTarget: Entry | null = null
  let checkDuplicatesSearchRoot = ''
  let checkDuplicatesDuplicates: string[] = []
  let checkDuplicatesScanning = false
  let checkDuplicatesProgressPercent = 0
  let checkDuplicatesProgressLabel = ''
  let checkDuplicatesError = ''
  let onChangeCheckDuplicatesSearchRoot: (value: string) => void = () => {}
  let onCopyCheckDuplicates: () => void | Promise<void> = () => {}
  let onSearchCheckDuplicates: () => void | Promise<void> = () => {}
  let onCloseCheckDuplicates: () => void = () => {}
  let newFolderOpen = false
  export let newFolderName = ''
  let newFolderError = ''
  let onConfirmNewFolder: () => void = () => {}
  let onCancelNewFolder: () => void = () => {}
  let newFileOpen = false
  export let newFileName = ''
  let newFileError = ''
  let newFileTypeHint = ''
  let onConfirmNewFile: () => void = () => {}
  let onCancelNewFile: () => void = () => {}

  let openWithOpen = false
  let openWithApps: OpenWithApp[] = []
  let openWithLoading = false
  let openWithError = ''
  let openWithBusy = false
  let onConfirmOpenWith: (choice: OpenWithChoice) => void = () => {}
  let onCloseOpenWith: () => void = () => {}

  let propertiesOpen = false
  let propertiesEntry: Entry | null = null
  let propertiesMutationsLocked = false
  let propertiesCount = 1
  let propertiesSize: number | null = null
  let propertiesItemCount: number | null = null
  let propertiesHidden: boolean | 'mixed' | null = null
  let propertiesExtraMetadataLoading = false
  let propertiesExtraMetadataError: string | null = null
  let propertiesExtraMetadata:
    | {
        kind: string
        sections: Array<{
          id: string
          title: string
          fields: Array<{ key: string; label: string; value: string }>
        }>
      }
    | null = null
  let propertiesPermissionsLoading = false
  let propertiesPermissionsApplying = false
  let propertiesOwnershipApplying = false
  let propertiesOwnershipError: string | null = null
  let propertiesOwnershipUsers: string[] = []
  let propertiesOwnershipGroups: string[] = []
  let propertiesOwnershipOptionsLoading = false
  let propertiesOwnershipOptionsError: string | null = null
  type AccessBit = boolean | 'mixed'
  type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
  let propertiesPermissions:
    | {
        accessSupported: boolean
        ownershipSupported: boolean
        ownerName: string | null
        groupName: string | null
        owner: Access | null
        group: Access | null
        other: Access | null
      }
    | null = null
  let onTogglePermissionsAccess: (scope: 'owner' | 'group' | 'other', key: 'read' | 'write' | 'exec', next: boolean) => void =
    () => {}
  let onSetOwnership: (owner: string, group: string) => void | Promise<void> = () => {}
  let onToggleHidden: (next: boolean) => void = () => {}
  let onLoadPropertiesExtraMetadata: () => void = () => {}
  let onCloseProperties: () => void = () => {}

  let bookmarkModalOpen = false
  let bookmarkCandidate: Entry | null = null
  export let bookmarkName = ''
  export let bookmarkInputEl: HTMLInputElement | null = null
  let onConfirmBookmark: () => void = () => {}
  let onCancelBookmark: () => void = () => {}

  let toastMessage: string | null = null

  type ExplorerShellSidebarProps = {
    collapsed: boolean
    places: typeof places
    bookmarks: typeof bookmarks
    partitions: typeof partitions
    onPlaceSelect: typeof onPlaceSelect
    onBookmarkSelect: typeof onBookmarkSelect
    onRemoveBookmark: typeof onRemoveBookmark
    dragTargetPath: typeof dragTargetPath
    onBookmarkDragOver: typeof onBookmarkDragOver
    onBookmarkDragLeave: typeof onBookmarkDragLeave
    onBookmarkDrop: typeof onBookmarkDrop
    onPartitionSelect: typeof onPartitionSelect
    onPartitionEject: typeof onPartitionEject
  }

  type ExplorerShellTopbarProps = {
    mode: typeof mode
    searchMode: typeof searchMode
    loading: typeof loading
    viewMode: typeof viewMode
    showHidden: typeof showHidden
    activity: typeof activity
    onFocus: typeof onFocus
    onBlur: typeof onBlur
    onSubmitPath: typeof onSubmitPath
    onSearch: typeof onSearch
    onExitSearch: typeof onExitSearch
    onNavigateSegment: typeof onNavigateSegment
    onTopbarAction: typeof onTopbarAction
    onTopbarViewModeChange: typeof onTopbarViewModeChange
  }

  type ExplorerShellListingProps = {
    noticeMessage: typeof noticeMessage
    searchRunning: typeof searchRunning
    filterActive: typeof filterActive
    filterValue: typeof filterValue
    currentPath: typeof currentPath
    cols: typeof cols
    gridTemplate: typeof gridTemplate
    filterSourceEntries: typeof filterSourceEntries
    filteredEntries: typeof filteredEntries
    visibleEntries: typeof visibleEntries
    columnFilters: typeof columnFilters
    columnFacets: typeof columnFacets
    columnFacetsLoading: typeof columnFacetsLoading
    onEnsureColumnFacets: typeof onEnsureColumnFacets
    start: typeof start
    offsetY: typeof offsetY
    totalHeight: typeof totalHeight
    wide: typeof wide
    selected: typeof selected
    sortField: typeof sortField
    sortDirection: typeof sortDirection
    isHidden: typeof isHidden
    displayName: typeof displayName
    formatSize: typeof formatSize
    formatItems: typeof formatItems
    clipboardMode: typeof clipboardMode
    clipboardPaths: typeof clipboardPaths
    onRowsScroll: typeof onRowsScroll
    onWheel: typeof onWheel
    onRowsKeydown: typeof onRowsKeydown
    onRowsMousedown: typeof onRowsMousedown
    onRowsClick: typeof onRowsClick
    onRowsContextMenu: typeof onRowsContextMenu
    onChangeSort: typeof onChangeSort
    onToggleFilter: typeof onToggleFilter
    onResetFilter: typeof onResetFilter
    onStartResize: typeof onStartResize
    ariaSort: typeof ariaSort
    onRowClick: typeof onRowClick
    onOpen: typeof onOpen
    onContextMenu: typeof onContextMenu
    onToggleStar: typeof onToggleStar
    onRowDragStart: typeof onRowDragStart
    onRowDragEnd: typeof onRowDragEnd
    onRowDragOver: typeof onRowDragOver
    onRowDragEnter: typeof onRowDragEnter
    onRowDrop: typeof onRowDrop
    onRowDragLeave: typeof onRowDragLeave
    dragTargetPath: typeof dragTargetPath
    dragAllowed: typeof dragAllowed
    dragging: typeof dragging
    onBreadcrumbDragOver: typeof onBreadcrumbDragOver
    onBreadcrumbDragLeave: typeof onBreadcrumbDragLeave
    onBreadcrumbDrop: typeof onBreadcrumbDrop
    selectionActive: typeof selectionActive
    selectionRect: typeof selectionRect
    videoThumbs: typeof videoThumbs
    thumbnailsEnabled: typeof thumbnailsEnabled
    thumbnailRefreshToken: typeof thumbnailRefreshToken
  }

  type ExplorerShellMenuProps = {
    contextMenu: typeof contextMenu
    blankMenu: typeof blankMenu
    onContextSelect: typeof onContextSelect
    onBlankContextSelect: typeof onBlankContextSelect
    onCloseContextMenu: typeof onCloseContextMenu
    onCloseBlankContextMenu: typeof onCloseBlankContextMenu
  }

  type ExplorerShellModalProps = {
    deleteConfirmOpen: typeof deleteConfirmOpen
    deleteTargets: typeof deleteTargets
    onConfirmDelete: typeof onConfirmDelete
    onCancelDelete: typeof onCancelDelete
    renameModalOpen: typeof renameModalOpen
    renameTarget: typeof renameTarget
    renameError: typeof renameError
    onConfirmRename: typeof onConfirmRename
    onCancelRename: typeof onCancelRename
    advancedRenameOpen: typeof advancedRenameOpen
    advancedRenameEntries: typeof advancedRenameEntries
    advancedRenameRegex: typeof advancedRenameRegex
    advancedRenameReplacement: typeof advancedRenameReplacement
    advancedRenamePrefix: typeof advancedRenamePrefix
    advancedRenameSuffix: typeof advancedRenameSuffix
    advancedRenameCaseSensitive: typeof advancedRenameCaseSensitive
    advancedRenameKeepExtension: typeof advancedRenameKeepExtension
    advancedRenameSequenceMode: typeof advancedRenameSequenceMode
    advancedRenameSequencePlacement: typeof advancedRenameSequencePlacement
    advancedRenameSequenceStart: typeof advancedRenameSequenceStart
    advancedRenameSequenceStep: typeof advancedRenameSequenceStep
    advancedRenameSequencePad: typeof advancedRenameSequencePad
    advancedRenameError: typeof advancedRenameError
    advancedRenamePreview: typeof advancedRenamePreview
    advancedRenamePreviewError: typeof advancedRenamePreviewError
    advancedRenamePreviewLoading: typeof advancedRenamePreviewLoading
    onAdvancedRenameChange: typeof onAdvancedRenameChange
    onConfirmAdvancedRename: typeof onConfirmAdvancedRename
    onCancelAdvancedRename: typeof onCancelAdvancedRename
    compressOpen: typeof compressOpen
    compressError: typeof compressError
    onConfirmCompress: typeof onConfirmCompress
    onCancelCompress: typeof onCancelCompress
    checkDuplicatesOpen: typeof checkDuplicatesOpen
    checkDuplicatesTarget: typeof checkDuplicatesTarget
    checkDuplicatesSearchRoot: typeof checkDuplicatesSearchRoot
    checkDuplicatesDuplicates: typeof checkDuplicatesDuplicates
    checkDuplicatesScanning: typeof checkDuplicatesScanning
    checkDuplicatesProgressPercent: typeof checkDuplicatesProgressPercent
    checkDuplicatesProgressLabel: typeof checkDuplicatesProgressLabel
    checkDuplicatesError: typeof checkDuplicatesError
    onChangeCheckDuplicatesSearchRoot: typeof onChangeCheckDuplicatesSearchRoot
    onCopyCheckDuplicates: typeof onCopyCheckDuplicates
    onSearchCheckDuplicates: typeof onSearchCheckDuplicates
    onCloseCheckDuplicates: typeof onCloseCheckDuplicates
    newFolderOpen: typeof newFolderOpen
    newFolderError: typeof newFolderError
    onConfirmNewFolder: typeof onConfirmNewFolder
    onCancelNewFolder: typeof onCancelNewFolder
    newFileOpen: typeof newFileOpen
    newFileError: typeof newFileError
    newFileTypeHint: typeof newFileTypeHint
    onConfirmNewFile: typeof onConfirmNewFile
    onCancelNewFile: typeof onCancelNewFile
    openWithOpen: typeof openWithOpen
    openWithApps: typeof openWithApps
    openWithLoading: typeof openWithLoading
    openWithError: typeof openWithError
    openWithBusy: typeof openWithBusy
    onConfirmOpenWith: typeof onConfirmOpenWith
    onCloseOpenWith: typeof onCloseOpenWith
    propertiesOpen: typeof propertiesOpen
    propertiesEntry: typeof propertiesEntry
    propertiesMutationsLocked: typeof propertiesMutationsLocked
    propertiesCount: typeof propertiesCount
    propertiesSize: typeof propertiesSize
    propertiesItemCount: typeof propertiesItemCount
    propertiesHidden: typeof propertiesHidden
    propertiesExtraMetadataLoading: typeof propertiesExtraMetadataLoading
    propertiesExtraMetadataError: typeof propertiesExtraMetadataError
    propertiesExtraMetadata: typeof propertiesExtraMetadata
    propertiesPermissionsLoading: typeof propertiesPermissionsLoading
    propertiesPermissionsApplying: typeof propertiesPermissionsApplying
    propertiesOwnershipApplying: typeof propertiesOwnershipApplying
    propertiesOwnershipError: typeof propertiesOwnershipError
    propertiesOwnershipUsers: typeof propertiesOwnershipUsers
    propertiesOwnershipGroups: typeof propertiesOwnershipGroups
    propertiesOwnershipOptionsLoading: typeof propertiesOwnershipOptionsLoading
    propertiesOwnershipOptionsError: typeof propertiesOwnershipOptionsError
    propertiesPermissions: typeof propertiesPermissions
    onTogglePermissionsAccess: typeof onTogglePermissionsAccess
    onSetOwnership: typeof onSetOwnership
    onToggleHidden: typeof onToggleHidden
    onLoadPropertiesExtraMetadata: typeof onLoadPropertiesExtraMetadata
    onCloseProperties: typeof onCloseProperties
    bookmarkModalOpen: typeof bookmarkModalOpen
    bookmarkCandidate: typeof bookmarkCandidate
    onConfirmBookmark: typeof onConfirmBookmark
    onCancelBookmark: typeof onCancelBookmark
    toastMessage: typeof toastMessage
  }

  type ExplorerShellStatusProps = {
    selectionText: typeof selectionText
  }

  export let sidebarProps: ExplorerShellSidebarProps | any = {}
  export let topbarProps: ExplorerShellTopbarProps | any = {}
  export let listingProps: ExplorerShellListingProps | any = {}
  export let menuProps: ExplorerShellMenuProps | any = {}
  export let modalProps: ExplorerShellModalProps | any = {}
  export let statusProps: ExplorerShellStatusProps | any = {}

  $: ({
    collapsed: sidebarCollapsed,
    places,
    bookmarks,
    partitions,
    onPlaceSelect,
    onBookmarkSelect,
    onRemoveBookmark,
    dragTargetPath,
    onBookmarkDragOver,
    onBookmarkDragLeave,
    onBookmarkDrop,
    onPartitionSelect,
    onPartitionEject,
  } = sidebarProps)

  $: ({
    mode,
    searchMode,
    loading,
    viewMode,
    showHidden,
    activity,
    onFocus,
    onBlur,
    onSubmitPath,
    onSearch,
    onExitSearch,
    onNavigateSegment,
    onTopbarAction,
    onTopbarViewModeChange,
  } = topbarProps)

  $: ({
    noticeMessage,
    searchRunning,
    filterActive,
    filterValue,
    currentPath,
    cols,
    gridTemplate,
    filterSourceEntries,
    filteredEntries,
    visibleEntries,
    columnFilters,
    columnFacets,
    columnFacetsLoading,
    onEnsureColumnFacets,
    start,
    offsetY,
    totalHeight,
    wide,
    selected,
    sortField,
    sortDirection,
    isHidden,
    displayName,
    formatSize,
    formatItems,
    clipboardMode,
    clipboardPaths,
    onRowsScroll,
    onWheel,
    onRowsKeydown,
    onRowsMousedown,
    onRowsClick,
    onRowsContextMenu,
    onChangeSort,
    onToggleFilter,
    onResetFilter,
    onStartResize,
    ariaSort,
    onRowClick,
    onOpen,
    onContextMenu,
    onToggleStar,
    onRowDragStart,
    onRowDragEnd,
    onRowDragOver,
    onRowDragEnter,
    onRowDrop,
    onRowDragLeave,
    dragTargetPath,
    dragAllowed,
    dragging,
    onBreadcrumbDragOver,
    onBreadcrumbDragLeave,
    onBreadcrumbDrop,
    selectionActive,
    selectionRect,
    videoThumbs,
    thumbnailsEnabled,
    thumbnailRefreshToken,
  } = listingProps)

  $: ({
    contextMenu,
    blankMenu,
    onContextSelect,
    onBlankContextSelect,
    onCloseContextMenu,
    onCloseBlankContextMenu,
  } = menuProps)

  $: ({
    deleteConfirmOpen,
    deleteTargets,
    onConfirmDelete,
    onCancelDelete,
    renameModalOpen,
    renameTarget,
    renameError,
    onConfirmRename,
    onCancelRename,
    advancedRenameOpen,
    advancedRenameEntries,
    advancedRenameRegex,
    advancedRenameReplacement,
    advancedRenamePrefix,
    advancedRenameSuffix,
    advancedRenameCaseSensitive,
    advancedRenameKeepExtension,
    advancedRenameSequenceMode,
    advancedRenameSequencePlacement,
    advancedRenameSequenceStart,
    advancedRenameSequenceStep,
    advancedRenameSequencePad,
    advancedRenameError,
    advancedRenamePreview,
    advancedRenamePreviewError,
    advancedRenamePreviewLoading,
    onAdvancedRenameChange,
    onConfirmAdvancedRename,
    onCancelAdvancedRename,
    compressOpen,
    compressError,
    onConfirmCompress,
    onCancelCompress,
    checkDuplicatesOpen,
    checkDuplicatesTarget,
    checkDuplicatesSearchRoot,
    checkDuplicatesDuplicates,
    checkDuplicatesScanning,
    checkDuplicatesProgressPercent,
    checkDuplicatesProgressLabel,
    checkDuplicatesError,
    onChangeCheckDuplicatesSearchRoot,
    onCopyCheckDuplicates,
    onSearchCheckDuplicates,
    onCloseCheckDuplicates,
    newFolderOpen,
    newFolderError,
    onConfirmNewFolder,
    onCancelNewFolder,
    newFileOpen,
    newFileError,
    newFileTypeHint,
    onConfirmNewFile,
    onCancelNewFile,
    openWithOpen,
    openWithApps,
    openWithLoading,
    openWithError,
    openWithBusy,
    onConfirmOpenWith,
    onCloseOpenWith,
    propertiesOpen,
    propertiesEntry,
    propertiesMutationsLocked,
    propertiesCount,
    propertiesSize,
    propertiesItemCount,
    propertiesHidden,
    propertiesExtraMetadataLoading,
    propertiesExtraMetadataError,
    propertiesExtraMetadata,
    propertiesPermissionsLoading,
    propertiesPermissionsApplying,
    propertiesOwnershipApplying,
    propertiesOwnershipError,
    propertiesOwnershipUsers,
    propertiesOwnershipGroups,
    propertiesOwnershipOptionsLoading,
    propertiesOwnershipOptionsError,
    propertiesPermissions,
    onTogglePermissionsAccess,
    onSetOwnership,
    onToggleHidden,
    onLoadPropertiesExtraMetadata,
    onCloseProperties,
    bookmarkModalOpen,
    bookmarkCandidate,
    onConfirmBookmark,
    onCancelBookmark,
    toastMessage,
  } = modalProps)

  $: ({ selectionText } = statusProps)

  $: hasActiveColumnFilters =
    columnFilters.name.size > 0 ||
    columnFilters.type.size > 0 ||
    columnFilters.modified.size > 0 ||
    columnFilters.size.size > 0

</script>

<main class="shell">
  <div class="layout" class:collapsed={sidebarCollapsed}>
    <Sidebar
      {places}
      {bookmarks}
      {partitions}
      collapsed={sidebarCollapsed}
      onPlaceSelect={onPlaceSelect}
      onBookmarkSelect={onBookmarkSelect}
      onRemoveBookmark={onRemoveBookmark}
      {dragTargetPath}
      onBookmarkDragOver={onBookmarkDragOver}
      onBookmarkDragLeave={onBookmarkDragLeave}
      onBookmarkDrop={onBookmarkDrop}
      onPartitionSelect={onPartitionSelect}
      onPartitionEject={onPartitionEject}
    />

    <section class="content">
      <Topbar
        bind:pathInput
        bind:pathInputEl
        {viewMode}
        {showHidden}
        {mode}
        {searchMode}
        {loading}
        {activity}
        onFocus={onFocus}
        onBlur={onBlur}
        onSubmitPath={onSubmitPath}
        onSearch={onSearch}
        onExitSearch={onExitSearch}
        onNavigateSegment={onNavigateSegment}
        onMainMenuAction={onTopbarAction}
        onToggleViewMode={onTopbarViewModeChange}
        dragTargetPath={dragTargetPath}
        onBreadcrumbDragOver={onBreadcrumbDragOver}
        onBreadcrumbDragLeave={onBreadcrumbDragLeave}
        onBreadcrumbDrop={onBreadcrumbDrop}
      />

      <Notice message={noticeMessage} />

      {#if loading && (searchRunning || filterActive)}
        <div class="pill">{filterActive ? 'Filtering' : 'Searching'}: "{filterValue}"</div>
      {/if}

      {#if viewMode === 'list'}
        <FileList
        {cols}
        {gridTemplate}
        bind:rowsEl
        bind:headerEl
        {loading}
        {searchMode}
        {columnFilters}
        {columnFacets}
        columnFacetsLoading={columnFacetsLoading}
        onEnsureColumnFacets={onEnsureColumnFacets}
        filterValue={filterValue}
        filterSourceEntries={filterSourceEntries}
        filteredEntries={filteredEntries}
        visibleEntries={visibleEntries}
        {start}
        {offsetY}
        {totalHeight}
          {wide}
          {selected}
          {sortField}
          {sortDirection}
          {isHidden}
          {displayName}
          {formatSize}
          {formatItems}
          {clipboardMode}
          {clipboardPaths}
          onRowsScroll={onRowsScroll}
          onWheel={onWheel}
          onRowsKeydown={onRowsKeydown}
          onRowsMousedown={onRowsMousedown}
          onRowsClick={onRowsClick}
        onRowsContextMenu={onRowsContextMenu}
        onChangeSort={onChangeSort}
        onToggleFilter={onToggleFilter}
        onResetFilter={onResetFilter}
        onStartResize={onStartResize}
        ariaSort={ariaSort}
          onRowClick={onRowClick}
          onOpen={onOpen}
          onContextMenu={onContextMenu}
          onToggleStar={onToggleStar}
          onRowDragStart={onRowDragStart}
          onRowDragEnd={onRowDragEnd}
          onRowDragEnter={onRowDragEnter}
          onRowDragOver={onRowDragOver}
          onRowDrop={onRowDrop}
          onRowDragLeave={onRowDragLeave}
          dragTargetPath={dragTargetPath}
          dragAllowed={dragAllowed}
          dragging={dragging}
          selectionActive={selectionActive}
          selectionRect={selectionRect}
        />
      {:else}
      {#if hasActiveColumnFilters}
        <div class="grid-filter-indicator">Column filters active</div>
      {/if}
      <FileGrid
        entries={filteredEntries}
        visibleEntries={visibleEntries}
        {start}
        {offsetY}
        {totalHeight}
        {currentPath}
        bind:rowsEl
        {thumbnailsEnabled}
        {selected}
        {isHidden}
        {displayName}
        videoThumbs={videoThumbs}
        {thumbnailRefreshToken}
        {clipboardMode}
          {clipboardPaths}
          onWheel={onWheel}
          onRowsContextMenu={onRowsContextMenu}
          onRowsClick={onRowsClick}
          onRowsMousedown={onRowsMousedown}
          onRowsScroll={onRowsScroll}
          onRowsKeydown={onRowsKeydown}
          onRowClick={onRowClick}
          onContextMenu={onContextMenu}
          onRowDragStart={onRowDragStart}
          onRowDragEnd={onRowDragEnd}
          onRowDragEnter={onRowDragEnter}
          onRowDragOver={onRowDragOver}
          onRowDrop={onRowDrop}
          onRowDragLeave={onRowDragLeave}
          {dragTargetPath}
          {dragAllowed}
          {dragging}
          selectionActive={selectionActive}
          selectionRect={selectionRect}
        />
      {/if}
      <Statusbar {selectionText} />
    </section>
  </div>
</main>

<ContextMenu
  open={contextMenu.open}
  x={contextMenu.x}
  y={contextMenu.y}
  actions={contextMenu.actions}
  onSelect={onContextSelect}
  onClose={onCloseContextMenu}
/>
<ContextMenu
  open={blankMenu.open}
  x={blankMenu.x}
  y={blankMenu.y}
  actions={blankMenu.actions}
  onSelect={onBlankContextSelect}
  onClose={onCloseBlankContextMenu}
/>
<DeleteConfirmModal
  open={deleteConfirmOpen}
  targetLabel={deleteTargets.length === 1 ? deleteTargets[0].path : `${deleteTargets.length} items`}
  onConfirm={onConfirmDelete}
  onCancel={onCancelDelete}
/>
<RenameModal
  open={renameModalOpen}
  entryName={renameTarget?.name ?? ''}
  bind:value={renameValue}
  error={renameError}
  onConfirm={onConfirmRename}
  onCancel={onCancelRename}
/>
<AdvancedRenameModal
  open={advancedRenameOpen}
  entries={advancedRenameEntries}
  regex={advancedRenameRegex}
  replacement={advancedRenameReplacement}
  prefix={advancedRenamePrefix}
  suffix={advancedRenameSuffix}
  caseSensitive={advancedRenameCaseSensitive}
  keepExtension={advancedRenameKeepExtension}
  sequenceMode={advancedRenameSequenceMode}
  sequencePlacement={advancedRenameSequencePlacement}
  sequenceStart={advancedRenameSequenceStart}
  sequenceStep={advancedRenameSequenceStep}
  sequencePad={advancedRenameSequencePad}
  error={advancedRenameError}
  preview={advancedRenamePreview}
  previewError={advancedRenamePreviewError}
  previewLoading={advancedRenamePreviewLoading}
  onChange={onAdvancedRenameChange}
  onConfirm={onConfirmAdvancedRename}
  onCancel={onCancelAdvancedRename}
/>
<CompressModal
  open={compressOpen}
  bind:value={compressName}
  bind:level={compressLevel}
  error={compressError}
  onConfirm={onConfirmCompress}
  onCancel={onCancelCompress}
/>
<CheckDuplicatesModal
  open={checkDuplicatesOpen}
  target={checkDuplicatesTarget}
  searchRoot={checkDuplicatesSearchRoot}
  duplicates={checkDuplicatesDuplicates}
  scanning={checkDuplicatesScanning}
  progressPercent={checkDuplicatesProgressPercent}
  progressLabel={checkDuplicatesProgressLabel}
  error={checkDuplicatesError}
  onChangeSearchRoot={onChangeCheckDuplicatesSearchRoot}
  onCopyList={onCopyCheckDuplicates}
  onSearch={onSearchCheckDuplicates}
  onClose={onCloseCheckDuplicates}
/>
<NewFolderModal
  open={newFolderOpen}
  bind:value={newFolderName}
  error={newFolderError}
  onConfirm={onConfirmNewFolder}
  onCancel={onCancelNewFolder}
/>
<NewFolderModal
  open={newFileOpen}
  bind:value={newFileName}
  error={newFileError}
  hint={newFileTypeHint}
  title="New file"
  inputId="new-file-name"
  onConfirm={onConfirmNewFile}
  onCancel={onCancelNewFile}
/>
<OpenWithModal
  open={openWithOpen}
  apps={openWithApps}
  loading={openWithLoading}
  error={openWithError}
  busy={openWithBusy}
  onConfirm={onConfirmOpenWith}
  onClose={onCloseOpenWith}
/>
<PropertiesModal
  open={propertiesOpen}
  entry={propertiesEntry}
  mutationsLocked={propertiesMutationsLocked}
  count={propertiesCount}
  size={propertiesSize}
  deepCount={propertiesItemCount}
  hidden={propertiesHidden}
  extraMetadataLoading={propertiesExtraMetadataLoading}
  extraMetadataError={propertiesExtraMetadataError}
  extraMetadata={propertiesExtraMetadata}
  permissionsLoading={propertiesPermissionsLoading}
  permissionsApplying={propertiesPermissionsApplying}
  ownershipApplying={propertiesOwnershipApplying}
  ownershipError={propertiesOwnershipError}
  ownershipUsers={propertiesOwnershipUsers}
  ownershipGroups={propertiesOwnershipGroups}
  ownershipOptionsLoading={propertiesOwnershipOptionsLoading}
  ownershipOptionsError={propertiesOwnershipOptionsError}
  permissions={propertiesPermissions}
  onToggleAccess={onTogglePermissionsAccess}
  onSetOwnership={onSetOwnership}
  onToggleHidden={onToggleHidden}
  onActivateExtra={onLoadPropertiesExtraMetadata}
  {formatSize}
  onClose={onCloseProperties}
/>

{#if bookmarkModalOpen}
  <BookmarkModal
    open={bookmarkModalOpen}
    entryName={bookmarkCandidate?.name ?? ''}
    bind:bookmarkName
    bind:bookmarkInputEl
    onConfirm={onConfirmBookmark}
    onCancel={onCancelBookmark}
  />
{/if}

<Toast message={toastMessage} />
