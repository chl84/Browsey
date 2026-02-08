<script lang="ts">
  import Sidebar from './Sidebar.svelte'
  import Topbar from './Topbar.svelte'
  import Notice from '../../../ui/Notice.svelte'
  import FileList from './FileList.svelte'
  import FileGrid from './FileGrid.svelte'
  import Statusbar from '../../../ui/Statusbar.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import DeleteConfirmModal from './DeleteConfirmModal.svelte'
  import RenameModal from './RenameModal.svelte'
  import NewFolderModal from './NewFolderModal.svelte'
  import OpenWithModal from './OpenWithModal.svelte'
  import PropertiesModal from './PropertiesModal.svelte'
  import AdvancedRenameModal from './AdvancedRenameModal.svelte'
  import BookmarkModal from './BookmarkModal.svelte'
  import Toast from '../../../ui/Toast.svelte'
  import CompressModal from './CompressModal.svelte'
  import type { Column, Entry, Partition, SortField } from '../types'
  import type { ContextAction } from '../hooks/useContextMenus'
  import type { OpenWithApp, OpenWithChoice } from '../services/openWith'
  import type { AdvancedRenamePayload } from '../modals/advancedRenameModal'

  export let sidebarCollapsed = false
  export let places: { label: string; path: string }[] = []
  export let bookmarks: { label: string; path: string }[] = []
  export let partitions: Partition[] = []
  export let onPlaceSelect: (label: string, path: string) => void = () => {}
  export let onBookmarkSelect: (path: string) => void = () => {}
  export let onRemoveBookmark: (path: string) => void = () => {}
  export let onPartitionSelect: (path: string) => void = () => {}
  export let onPartitionEject: (path: string) => void = () => {}

  export let pathInput = ''
  export let pathInputEl: HTMLInputElement | null = null
  export let mode: 'address' | 'filter' | 'search' = 'address'
  export let searchMode = false
  export let loading = false
  export let viewMode: 'list' | 'grid' = 'list'
  export let onFocus: () => void = () => {}
  export let onBlur: () => void = () => {}
  export let onSubmitPath: () => void = () => {}
  export let onSearch: () => void = () => {}
  export let onExitSearch: () => void = () => {}
  export let onNavigateSegment: (path: string) => void = () => {}

  export let noticeMessage = ''
  export let searchActive = false
  export let filterValue = ''
  export let currentPath = ''

  export let cols: Column[] = []
  export let gridTemplate = ''
  export let rowsEl: HTMLDivElement | null = null
  export let headerEl: HTMLDivElement | null = null
  export let filteredEntries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let start = 0
  export let offsetY = 0
  export let totalHeight = 0
  export let wide = false
  export let selected: Set<string> = new Set()
  export let sortField: SortField = 'name'
  export let sortDirection: 'asc' | 'desc' = 'asc'
  export let isHidden: (entry: Entry) => boolean = () => false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let formatSize: (n: number | null | undefined) => string = (n) => String(n ?? '')
  export let formatItems: (n?: number | null) => string = (n) => String(n ?? '')
  export let clipboardMode: 'copy' | 'cut' = 'copy'
  export let clipboardPaths: Set<string> = new Set()
  export let onRowsScroll: (e: Event) => void = () => {}
  export let onWheel: (e: WheelEvent) => void = () => {}
  export let onRowsKeydown: (e: KeyboardEvent) => void = () => {}
  export let onRowsMousedown: (e: MouseEvent) => void = () => {}
  export let onRowsClick: (e: MouseEvent) => void = () => {}
  export let onRowsContextMenu: (e: MouseEvent) => void = () => {}
  export let onChangeSort: (field: SortField) => void = () => {}
  export let onStartResize: (col: number, event: PointerEvent) => void = () => {}
  export let ariaSort: (field: SortField) => 'ascending' | 'descending' | 'none' = () => 'none'
  export let onRowClick: (entry: Entry, absoluteIndex: number, event: MouseEvent) => void = () => {}
  export let onOpen: (entry: Entry) => void = () => {}
  export let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
  export let onToggleStar: (entry: Entry) => void = () => {}
  export let onRowDragStart: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnd: (event: DragEvent) => void = () => {}
  export let onRowDragOver: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnter: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDrop: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragLeave: (entry: Entry, event: DragEvent) => void = () => {}

  export let selectionText = ''
  export let activity:
    | { label: string; percent: number | null; cancel?: (() => void) | null; cancelling?: boolean }
    | null = null
  export let selectionActive = false
  export let selectionRect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
  export let dragTargetPath: string | null = null
  export let dragAllowed = false
  export let dragging = false
  export let videoThumbs = true

  export let contextMenu: { open: boolean; x: number; y: number; actions: ContextAction[] } = {
    open: false,
    x: 0,
    y: 0,
    actions: [],
  }
  export let blankMenu: { open: boolean; x: number; y: number; actions: ContextAction[] } = {
    open: false,
    x: 0,
    y: 0,
    actions: [],
  }
  export let onContextSelect: (id: string) => void = () => {}
  export let onBlankContextSelect: (id: string) => void = () => {}
  export let onCloseContextMenu: () => void = () => {}
  export let onCloseBlankContextMenu: () => void = () => {}
  export let onBreadcrumbDragOver: (path: string, e: DragEvent) => void = () => {}
  export let onBreadcrumbDragLeave: (path: string, e: DragEvent) => void = () => {}
  export let onBreadcrumbDrop: (path: string, e: DragEvent) => void = () => {}

  export let deleteConfirmOpen = false
  export let deleteTargets: Entry[] = []
  export let onConfirmDelete: () => void = () => {}
  export let onCancelDelete: () => void = () => {}

  export let renameModalOpen = false
  export let renameTarget: Entry | null = null
  export let renameValue = ''
  export let renameError = ''
  export let onConfirmRename: (name: string) => void = () => {}
  export let onCancelRename: () => void = () => {}
  export let advancedRenameOpen = false
  export let advancedRenameEntries: Entry[] = []
  export let advancedRenameRegex = ''
  export let advancedRenameReplacement = ''
  export let advancedRenamePrefix = ''
  export let advancedRenameSuffix = ''
  export let advancedRenameCaseSensitive = true
  export let advancedRenameSequenceMode: 'none' | 'numeric' | 'alpha' = 'none'
  export let advancedRenameSequenceStart = 1
  export let advancedRenameSequenceStep = 1
  export let advancedRenameSequencePad = 2
  export let advancedRenameError = ''
  export let onAdvancedRenameChange: (payload: AdvancedRenamePayload) => void = () => {}
  export let onConfirmAdvancedRename: () => void = () => {}
  export let onCancelAdvancedRename: () => void = () => {}
  export let compressOpen = false
  export let compressName = ''
  export let compressLevel = 6
  export let compressError = ''
  export let onConfirmCompress: (name: string, level: number) => void = () => {}
  export let onCancelCompress: () => void = () => {}
  export let newFolderOpen = false
  export let newFolderName = ''
  export let newFolderError = ''
  export let onConfirmNewFolder: () => void = () => {}
  export let onCancelNewFolder: () => void = () => {}
  export let newFileOpen = false
  export let newFileName = ''
  export let newFileError = ''
  export let onConfirmNewFile: () => void = () => {}
  export let onCancelNewFile: () => void = () => {}

  export let openWithOpen = false
  export let openWithApps: OpenWithApp[] = []
  export let openWithLoading = false
  export let openWithError = ''
  export let openWithBusy = false
  export let onConfirmOpenWith: (choice: OpenWithChoice) => void = () => {}
  export let onCloseOpenWith: () => void = () => {}

  export let propertiesOpen = false
  export let propertiesEntry: Entry | null = null
  export let propertiesCount = 1
  export let propertiesSize: number | null = null
  export let propertiesItemCount: number | null = null
  export let propertiesHidden: boolean | 'mixed' | null = null
  type AccessBit = boolean | 'mixed'
  type Access = { read: AccessBit; write: AccessBit; exec: AccessBit }
  export let propertiesPermissions:
    | {
        accessSupported: boolean
        executableSupported: boolean
        readOnly: AccessBit | null
        executable: AccessBit | null
        owner: Access | null
        group: Access | null
        other: Access | null
      }
    | null = null
  export let onTogglePermissionsAccess: (scope: 'owner' | 'group' | 'other', key: 'read' | 'write' | 'exec', next: boolean) => void =
    () => {}
  export let onToggleHidden: (next: boolean) => void = () => {}
  export let onCloseProperties: () => void = () => {}

  export let bookmarkModalOpen = false
  export let bookmarkCandidate: Entry | null = null
  export let bookmarkName = ''
  export let bookmarkInputEl: HTMLInputElement | null = null
  export let onConfirmBookmark: () => void = () => {}
  export let onCancelBookmark: () => void = () => {}

  export let toastMessage: string | null = null

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
      onPartitionSelect={onPartitionSelect}
      onPartitionEject={onPartitionEject}
    />

    <section class="content">
      <Topbar
        bind:pathInput
        bind:pathInputEl
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
        dragTargetPath={dragTargetPath}
        onBreadcrumbDragOver={onBreadcrumbDragOver}
        onBreadcrumbDragLeave={onBreadcrumbDragLeave}
        onBreadcrumbDrop={onBreadcrumbDrop}
      />

      <Notice message={noticeMessage} />

      {#if searchActive && loading}
        <div class="pill">{mode === 'filter' ? 'Filtering' : 'Searching'}: "{filterValue}"</div>
      {/if}

      {#if viewMode === 'list'}
        <FileList
          {cols}
          {gridTemplate}
          bind:rowsEl
          bind:headerEl
          {loading}
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
      <FileGrid
        entries={filteredEntries}
        visibleEntries={visibleEntries}
        {start}
        {offsetY}
        {totalHeight}
        {currentPath}
        bind:rowsEl
        {selected}
        {isHidden}
        {displayName}
        videoThumbs={videoThumbs}
        {clipboardMode}
          {clipboardPaths}
          onWheel={onWheel}
          onRowsContextMenu={onRowsContextMenu}
          onRowsClick={onRowsClick}
          onRowsMousedown={onRowsMousedown}
          onRowsScroll={onRowsScroll}
          onRowsKeydown={onRowsKeydown}
          onOpen={onOpen}
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
  sequenceMode={advancedRenameSequenceMode}
  sequenceStart={advancedRenameSequenceStart}
  sequenceStep={advancedRenameSequenceStep}
  sequencePad={advancedRenameSequencePad}
  error={advancedRenameError}
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
  count={propertiesCount}
  size={propertiesSize}
  deepCount={propertiesItemCount}
  hidden={propertiesHidden}
  permissions={propertiesPermissions}
  onToggleAccess={onTogglePermissionsAccess}
  onToggleHidden={onToggleHidden}
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
