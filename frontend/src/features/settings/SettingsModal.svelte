<script lang="ts">
  import ModalShell from '../../shared/ui/ModalShell.svelte'
  import ConfirmActionModal from '../../shared/ui/ConfirmActionModal.svelte'
  import { onMount } from 'svelte'
  import type { DefaultSortField, Density } from '@/features/explorer'
  import type { ShortcutBinding, ShortcutCommandId } from '@/features/shortcuts'
  import { DEFAULT_SETTINGS, type Settings } from './settingsTypes'
  import { createSettingsModalViewModel } from './hooks/useSettingsModalViewModel'
  import GeneralSection from './sections/GeneralSection.svelte'
  import SortingSection from './sections/SortingSection.svelte'
  import AppearanceSection from './sections/AppearanceSection.svelte'
  import ArchivesSection from './sections/ArchivesSection.svelte'
  import ThumbnailsSection from './sections/ThumbnailsSection.svelte'
  import ShortcutsSection from './sections/ShortcutsSection.svelte'
  import PerformanceSection from './sections/PerformanceSection.svelte'
  import InteractionSection from './sections/InteractionSection.svelte'
  import DataSection from './sections/DataSection.svelte'
  import AccessibilitySection from './sections/AccessibilitySection.svelte'
  import AdvancedSection from './sections/AdvancedSection.svelte'
  import './SettingsModal.css'

  export let open = false
  export let onClose: () => void
  export let initialFilter = ''
  export let defaultViewValue: 'list' | 'grid' = 'list'
  export let showHiddenValue = true
  export let hiddenFilesLastValue = false
  export let foldersFirstValue = true
  export let confirmDeleteValue = true
  export let densityValue: Density = 'cozy'
  export let archiveNameValue = 'Archive'
  export let archiveLevelValue = 6
  export let openDestAfterExtractValue = true
  export let videoThumbsValue = true
  export let hardwareAccelerationValue = true
  export let ffmpegPathValue = ''
  export let thumbCacheMbValue = 300
  export let mountsPollMsValue = 8000
  export let doubleClickMsValue = 300
  export let sortFieldValue: DefaultSortField = 'name'
  export let sortDirectionValue: 'asc' | 'desc' = 'asc'
  export let startDirValue = '~'
  export let shortcuts: ShortcutBinding[] = []
  export let onChangeDefaultView: (value: 'list' | 'grid') => void = () => {}
  export let onToggleShowHidden: (value: boolean) => void = () => {}
  export let onToggleHiddenFilesLast: (value: boolean) => void = () => {}
  export let onToggleFoldersFirst: (value: boolean) => void = () => {}
  export let onToggleConfirmDelete: (value: boolean) => void = () => {}
  export let onChangeSortField: (value: typeof sortFieldValue) => void = () => {}
  export let onChangeSortDirection: (value: typeof sortDirectionValue) => void = () => {}
  export let onChangeStartDir: (value: string) => void = () => {}
  export let onChangeDensity: (value: Density) => void = () => {}
  export let onChangeArchiveName: (value: string) => void = () => {}
  export let onChangeArchiveLevel: (value: number) => void = () => {}
  export let onToggleOpenDestAfterExtract: (value: boolean) => void = () => {}
  export let onToggleVideoThumbs: (value: boolean) => void = () => {}
  export let onToggleHardwareAcceleration: (value: boolean) => void = () => {}
  export let onChangeFfmpegPath: (value: string) => void = () => {}
  export let onChangeThumbCacheMb: (value: number) => void = () => {}
  export let onChangeMountsPollMs: (value: number) => void = () => {}
  export let onChangeDoubleClickMs: (value: number) => void = () => {}
  export let onClearThumbCache: () => Promise<void> | void = () => {}
  export let onClearStars: () => Promise<void> | void = () => {}
  export let onClearBookmarks: () => Promise<void> | void = () => {}
  export let onClearRecents: () => Promise<void> | void = () => {}
  export let onChangeShortcut: (
    commandId: ShortcutCommandId,
    accelerator: string,
  ) => Promise<void> | void = () => {}

  let settings: Settings = { ...DEFAULT_SETTINGS }

  const patchSettings = (patch: Partial<Settings>) => {
    settings = { ...settings, ...patch }
  }

  const {
    filter,
    needle,
    setShortcuts,
    syncModalState,
    clearTarget,
    clearBusy,
    clearDialog,
    clearDialogMessage,
    requestClear,
    cancelClear,
    confirmClear,
    shortcutCaptureId,
    shortcutCaptureBusy,
    shortcutCaptureError,
    beginShortcutCapture,
    handleWindowKeydown,
    restoreDefaults,
    buildFilterModel,
  } = createSettingsModalViewModel({
    onClose: () => onClose(),
    onChangeShortcut: (commandId, accelerator) => onChangeShortcut(commandId, accelerator),
    onClearThumbCache: () => onClearThumbCache(),
    onClearStars: () => onClearStars(),
    onClearBookmarks: () => onClearBookmarks(),
    onClearRecents: () => onClearRecents(),
  })

  let filterModel = buildFilterModel(settings)

  $: syncModalState(open, initialFilter)
  $: setShortcuts(shortcuts)
  $: if (settings.defaultView !== defaultViewValue) {
    patchSettings({ defaultView: defaultViewValue })
  }
  $: if (settings.showHidden !== showHiddenValue) {
    patchSettings({ showHidden: showHiddenValue })
  }
  $: if (settings.hiddenFilesLast !== hiddenFilesLastValue) {
    patchSettings({ hiddenFilesLast: hiddenFilesLastValue })
  }
  $: if (settings.foldersFirst !== foldersFirstValue) {
    patchSettings({ foldersFirst: foldersFirstValue })
  }
  $: if (settings.startDir !== startDirValue) {
    patchSettings({ startDir: startDirValue })
  }
  $: if (settings.confirmDelete !== confirmDeleteValue) {
    patchSettings({ confirmDelete: confirmDeleteValue })
  }
  $: if (settings.sortField !== sortFieldValue) {
    patchSettings({ sortField: sortFieldValue })
  }
  $: if (settings.sortDirection !== sortDirectionValue) {
    patchSettings({ sortDirection: sortDirectionValue })
  }
  $: if (settings.density !== densityValue) {
    patchSettings({ density: densityValue })
  }
  $: if (settings.archiveName !== archiveNameValue) {
    patchSettings({ archiveName: archiveNameValue })
  }
  $: if (settings.archiveLevel !== archiveLevelValue) {
    patchSettings({ archiveLevel: archiveLevelValue })
  }
  $: if (settings.openDestAfterExtract !== openDestAfterExtractValue) {
    patchSettings({ openDestAfterExtract: openDestAfterExtractValue })
  }
  $: if (settings.videoThumbs !== videoThumbsValue) {
    patchSettings({ videoThumbs: videoThumbsValue })
  }
  $: if (settings.hardwareAcceleration !== hardwareAccelerationValue) {
    patchSettings({ hardwareAcceleration: hardwareAccelerationValue })
  }
  $: if (settings.ffmpegPath !== ffmpegPathValue) {
    patchSettings({ ffmpegPath: ffmpegPathValue })
  }
  $: if (settings.thumbCacheMb !== thumbCacheMbValue) {
    patchSettings({ thumbCacheMb: thumbCacheMbValue })
  }
  $: if (settings.mountsPollMs !== mountsPollMsValue) {
    patchSettings({ mountsPollMs: mountsPollMsValue })
  }
  $: if (settings.doubleClickMs !== doubleClickMsValue) {
    patchSettings({ doubleClickMs: doubleClickMsValue })
  }

  $: {
    $needle
    shortcuts
    settings
    filterModel = buildFilterModel(settings)
  }

  const handleRestoreDefaults = () => {
    restoreDefaults((next) => {
      settings = next
    })
  }

  onMount(() => {
    const listener = (e: KeyboardEvent) => handleWindowKeydown(e, open)
    window.addEventListener('keydown', listener, { capture: true })
    return () => {
      window.removeEventListener('keydown', listener, { capture: true } as any)
    }
  })

</script>

{#if open}
  <ModalShell
    title={null}
    open={open}
    onClose={onClose}
    modalClass="settings-modal"
    modalWidth="720px"
    initialFocusSelector=".settings-filter"
  >
    <svelte:fragment slot="header">
      <div class="settings-header">
        <h2>Settings</h2>
        <input class="settings-filter" type="search" placeholder="Filter settings" bind:value={$filter} />
        <button type="button" class="restore-btn" on:click={handleRestoreDefaults}>Restore defaults</button>
      </div>
    </svelte:fragment>

    <div class="settings-panel single">
      <div class="form-rows settings-table">
        <GeneralSection
          show={filterModel.showGeneral}
          showDefaultViewRow={filterModel.showDefaultViewRow}
          showFoldersFirstRow={filterModel.showFoldersFirstRow}
          showShowHiddenRow={filterModel.showShowHiddenRow}
          showHiddenFilesLastRow={filterModel.showHiddenFilesLastRow}
          showStartDirRow={filterModel.showStartDirRow}
          showConfirmDeleteRow={filterModel.showConfirmDeleteRow}
          hiddenFilesLastDisabled={filterModel.hiddenFilesLastDisabled}
          {settings}
          onPatch={patchSettings}
          {onChangeDefaultView}
          {onToggleFoldersFirst}
          {onToggleShowHidden}
          {onToggleHiddenFilesLast}
          {onChangeStartDir}
          {onToggleConfirmDelete}
        />

        <SortingSection
          show={filterModel.showSorting}
          showSortFieldRow={filterModel.showSortFieldRow}
          showSortDirectionRow={filterModel.showSortDirectionRow}
          {settings}
          onPatch={patchSettings}
          {onChangeSortField}
          {onChangeSortDirection}
        />

        <AppearanceSection
          show={filterModel.showAppearance}
          showDensityRow={filterModel.showDensityRow}
          {settings}
          onPatch={patchSettings}
          {onChangeDensity}
        />

        <ArchivesSection
          show={filterModel.showArchives}
          showArchiveNameRow={filterModel.showArchiveNameRow}
          showArchiveLevelRow={filterModel.showArchiveLevelRow}
          showAfterExtractRow={filterModel.showAfterExtractRow}
          showRarNoteRow={filterModel.showRarNoteRow}
          {settings}
          onPatch={patchSettings}
          {onChangeArchiveName}
          {onChangeArchiveLevel}
          {onToggleOpenDestAfterExtract}
        />

        <ThumbnailsSection
          show={filterModel.showThumbnails}
          showVideoThumbsRow={filterModel.showVideoThumbsRow}
          showFfmpegPathRow={filterModel.showFfmpegPathRow}
          showThumbCacheRow={filterModel.showThumbCacheRow}
          thumbsDisabled={filterModel.thumbsDisabled}
          {settings}
          onPatch={patchSettings}
          {onToggleVideoThumbs}
          {onChangeFfmpegPath}
          {onChangeThumbCacheMb}
        />

        <ShortcutsSection
          show={filterModel.showShortcuts}
          shortcutColumns={filterModel.shortcutColumns}
          shortcutCaptureId={$shortcutCaptureId}
          shortcutCaptureBusy={$shortcutCaptureBusy}
          shortcutCaptureError={$shortcutCaptureError}
          onBeginShortcutCapture={beginShortcutCapture}
        />

        <PerformanceSection
          show={filterModel.showPerformance}
          showHardwareAccelerationRow={filterModel.showHardwareAccelerationRow}
          showMountsPollRow={filterModel.showMountsPollRow}
          {settings}
          onPatch={patchSettings}
          {onToggleHardwareAcceleration}
          {onChangeMountsPollMs}
        />

        <InteractionSection
          show={filterModel.showInteraction}
          showDoubleClickRow={filterModel.showDoubleClickRow}
          {settings}
          onPatch={patchSettings}
          {onChangeDoubleClickMs}
        />

        <DataSection
          show={filterModel.showData}
          clearBusy={$clearBusy}
          clearTarget={$clearTarget}
          onRequestClear={requestClear}
        />

        <AccessibilitySection
          show={filterModel.showAccessibility}
          showHighContrastRow={filterModel.showHighContrastRow}
          showScrollbarWidthRow={filterModel.showScrollbarWidthRow}
          {settings}
          onPatch={patchSettings}
        />

        <AdvancedSection
          show={filterModel.showAdvanced}
          showExternalToolsRow={filterModel.showExternalToolsRow}
          showLogLevelRow={filterModel.showLogLevelRow}
          {settings}
          onPatch={patchSettings}
        />
      </div>
    </div>
  </ModalShell>

  <ConfirmActionModal
    open={$clearTarget !== null}
    title={$clearDialog?.title ?? 'Confirm action'}
    message={$clearDialogMessage}
    confirmLabel={$clearDialog?.confirmLabel ?? 'Confirm'}
    cancelLabel="Cancel"
    danger={true}
    busy={$clearBusy}
    onConfirm={() => void confirmClear()}
    onCancel={cancelClear}
  />
{/if}
