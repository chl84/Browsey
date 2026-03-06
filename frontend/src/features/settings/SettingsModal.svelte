<script lang="ts">
  import ModalShell from '../../shared/ui/ModalShell.svelte'
  import ConfirmActionModal from '../../shared/ui/ConfirmActionModal.svelte'
  import TextField from '../../shared/ui/TextField.svelte'
  import { getErrorMessage } from '@/shared/lib/error'
  import { onMount } from 'svelte'
  import { loadCloudSetupStatus, type CloudSetupStatus } from '@/features/network'
  import type { DefaultSortField, Density } from '@/features/explorer'
  import type { ShortcutBinding, ShortcutCommandId } from '@/features/shortcuts'
  import { createDebouncedAsyncRunner } from './cloudSetup'
  import { DEFAULT_SETTINGS, restoreDefaultsCopy, type Settings } from './settingsTypes'
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
  import CloudSection from './sections/CloudSection.svelte'
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
  export let highContrastValue = false
  export let archiveNameValue = 'Archive'
  export let archiveLevelValue = 6
  export let openDestAfterExtractValue = true
  export let videoThumbsValue = true
  export let cloudThumbsValue = false
  export let cloudEnabledValue = true
  export let hardwareAccelerationValue = false
  export let ffmpegPathValue = ''
  export let thumbCacheMbValue = 300
  export let mountsPollMsValue = 8000
  export let doubleClickMsValue = 300
  export let logLevelValue: Settings['logLevel'] = 'warn'
  export let scrollbarWidthValue = 10
  export let rclonePathValue = ''
  export let sortFieldValue: DefaultSortField = 'name'
  export let sortDirectionValue: 'asc' | 'desc' = 'asc'
  export let startDirValue = '~'
  export let shortcuts: ShortcutBinding[] = []
  export let onChangeDefaultView: (value: 'list' | 'grid') => void = () => {}
  export let onToggleShowHidden: (value: boolean) => void = () => {}
  export let onToggleHiddenFilesLast: (value: boolean) => void = () => {}
  export let onToggleHighContrast: (value: boolean) => void = () => {}
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
  export let onToggleCloudThumbs: (value: boolean) => void = () => {}
  export let onToggleCloudEnabled: (value: boolean) => Promise<void> | void = () => {}
  export let onToggleHardwareAcceleration: (value: boolean) => void = () => {}
  export let onChangeFfmpegPath: (value: string) => void = () => {}
  export let onChangeThumbCacheMb: (value: number) => void = () => {}
  export let onChangeMountsPollMs: (value: number) => void = () => {}
  export let onChangeDoubleClickMs: (value: number) => void = () => {}
  export let onChangeLogLevel: (value: Settings['logLevel']) => void = () => {}
  export let onChangeScrollbarWidth: (value: number) => void = () => {}
  export let onChangeRclonePath: (value: string) => Promise<void> | void = () => {}
  export let onRestoreDefaults: () => Promise<void> | void = () => {}
  export let onClearThumbCache: () => Promise<void> | void = () => {}
  export let onClearCloudOpenCache: () => Promise<void> | void = () => {}
  export let onClearStars: () => Promise<void> | void = () => {}
  export let onClearBookmarks: () => Promise<void> | void = () => {}
  export let onClearRecents: () => Promise<void> | void = () => {}
  export let onChangeShortcut: (
    commandId: ShortcutCommandId,
    accelerator: string,
  ) => Promise<void> | void = () => {}

  let settings: Settings = { ...DEFAULT_SETTINGS }
  let cloudSetupStatus: CloudSetupStatus | null = null
  let cloudSetupStatusBusy = false
  let cloudSetupStatusError = ''
  let lastCloudSetupRequestId = 0
  let lastAppliedCloudEnabledValue = DEFAULT_SETTINGS.cloudEnabled
  let lastAppliedRclonePathValue = DEFAULT_SETTINGS.rclonePath
  let wasOpen = false

  const patchSettings = (patch: Partial<Settings>) => {
    settings = { ...settings, ...patch }
  }

  const refreshCloudSetupStatus = async () => {
    if (!settings.cloudEnabled) {
      cloudSetupStatusBusy = false
      cloudSetupStatusError = ''
      return
    }
    const requestId = ++lastCloudSetupRequestId
    cloudSetupStatusBusy = true
    try {
      const next = await loadCloudSetupStatus()
      if (requestId !== lastCloudSetupRequestId) return
      cloudSetupStatus = next
      cloudSetupStatusError = ''
    } catch (err) {
      if (requestId !== lastCloudSetupRequestId) return
      cloudSetupStatusError = getErrorMessage(err)
    } finally {
      if (requestId === lastCloudSetupRequestId) {
        cloudSetupStatusBusy = false
      }
    }
  }

  const rclonePathBlurRefresh = createDebouncedAsyncRunner(async (value: string) => {
    await onChangeRclonePath(value)
    lastAppliedRclonePathValue = value
    await refreshCloudSetupStatus()
  })

  const handleRclonePathBlur = (value: string) => {
    rclonePathBlurRefresh.schedule(value)
  }

  const handleCloudEnabledChange = async (value: boolean) => {
    await onToggleCloudEnabled(value)
    lastAppliedCloudEnabledValue = value
    if (!value) {
      rclonePathBlurRefresh.cancel()
      cloudSetupStatusBusy = false
      cloudSetupStatusError = ''
      return
    }
    await refreshCloudSetupStatus()
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
    restoreDefaultsOpen,
    restoreDefaultsBusy,
    restoreDefaultsDialogMessage,
    requestRestoreDefaults,
    cancelRestoreDefaults,
    confirmRestoreDefaults,
    shortcutCaptureId,
    shortcutCaptureBusy,
    shortcutCaptureError,
    beginShortcutCapture,
    handleWindowKeydown,
    buildFilterModel,
  } = createSettingsModalViewModel({
    onClose: () => onClose(),
    onChangeShortcut: (commandId, accelerator) => onChangeShortcut(commandId, accelerator),
    onRestoreDefaults: () => onRestoreDefaults(),
    onClearThumbCache: () => onClearThumbCache(),
    onClearCloudOpenCache: () => onClearCloudOpenCache(),
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
  $: if (settings.highContrast !== highContrastValue) {
    patchSettings({ highContrast: highContrastValue })
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
  $: if (settings.cloudThumbs !== cloudThumbsValue) {
    patchSettings({ cloudThumbs: cloudThumbsValue })
  }
  $: if (settings.cloudEnabled !== cloudEnabledValue && cloudEnabledValue !== lastAppliedCloudEnabledValue) {
    patchSettings({ cloudEnabled: cloudEnabledValue })
    lastAppliedCloudEnabledValue = cloudEnabledValue
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
  $: if (settings.logLevel !== logLevelValue) {
    patchSettings({ logLevel: logLevelValue })
  }
  $: if (settings.scrollbarWidth !== scrollbarWidthValue) {
    patchSettings({ scrollbarWidth: scrollbarWidthValue })
  }
  $: if (rclonePathValue !== lastAppliedRclonePathValue) {
    patchSettings({ rclonePath: rclonePathValue })
    lastAppliedRclonePathValue = rclonePathValue
  }
  $: {
    if (open && !wasOpen) {
      void refreshCloudSetupStatus()
    } else if (!open && wasOpen) {
      rclonePathBlurRefresh.cancel()
    }
    wasOpen = open
  }

  $: {
    $needle
    shortcuts
    settings
    filterModel = buildFilterModel(settings)
  }

  const handleRestoreDefaults = () => {
    requestRestoreDefaults()
  }

  const handleConfirmRestoreDefaults = async () => {
    await confirmRestoreDefaults((next) => {
      settings = next
    })
  }

  onMount(() => {
    const listener = (e: KeyboardEvent) => handleWindowKeydown(e, open)
    window.addEventListener('keydown', listener, { capture: true })
    return () => {
      rclonePathBlurRefresh.cancel()
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
        <TextField
          className="settings-filter"
          type="search"
          placeholder="Filter settings"
          bind:value={$filter}
        />
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
          showCloudThumbsRow={filterModel.showCloudThumbsRow}
          showFfmpegPathRow={filterModel.showFfmpegPathRow}
          showThumbCacheRow={filterModel.showThumbCacheRow}
          thumbsDisabled={filterModel.thumbsDisabled}
          {settings}
          onPatch={patchSettings}
          {onToggleVideoThumbs}
          {onToggleCloudThumbs}
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
          {onToggleHighContrast}
          {onChangeScrollbarWidth}
        />

        <CloudSection
          show={filterModel.showCloud}
          showCloudEnabledRow={filterModel.showCloudEnabledRow}
          showRclonePathRow={filterModel.showRclonePathRow}
          {settings}
          {cloudSetupStatus}
          {cloudSetupStatusBusy}
          {cloudSetupStatusError}
          onPatch={patchSettings}
          onToggleCloudEnabled={handleCloudEnabledChange}
          onChangeRclonePath={handleRclonePathBlur}
        />

        <AdvancedSection
          show={filterModel.showAdvanced}
          showLogLevelRow={filterModel.showLogLevelRow}
          {settings}
          onPatch={patchSettings}
          {onChangeLogLevel}
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

  <ConfirmActionModal
    open={$restoreDefaultsOpen}
    title={restoreDefaultsCopy.title}
    message={$restoreDefaultsDialogMessage}
    confirmLabel={restoreDefaultsCopy.confirmLabel}
    cancelLabel="Cancel"
    danger={false}
    busy={$restoreDefaultsBusy}
    onConfirm={() => void handleConfirmRestoreDefaults()}
    onCancel={cancelRestoreDefaults}
  />
{/if}
