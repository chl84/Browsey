import type { ComponentProps } from 'svelte'
import { SettingsModal } from '@/features/settings'

type SettingsModalProps = ComponentProps<typeof SettingsModal>

type Params = {
  settingsInitialFilter: SettingsModalProps['initialFilter']
  defaultViewPref: SettingsModalProps['defaultViewValue']
  showHidden: SettingsModalProps['showHiddenValue']
  hiddenFilesLast: SettingsModalProps['hiddenFilesLastValue']
  foldersFirst: SettingsModalProps['foldersFirstValue']
  confirmDelete: SettingsModalProps['confirmDeleteValue']
  density: SettingsModalProps['densityValue']
  highContrast: SettingsModalProps['highContrastValue']
  archiveName: SettingsModalProps['archiveNameValue']
  archiveLevel: SettingsModalProps['archiveLevelValue']
  openDestAfterExtract: SettingsModalProps['openDestAfterExtractValue']
  videoThumbs: SettingsModalProps['videoThumbsValue']
  hardwareAcceleration: SettingsModalProps['hardwareAccelerationValue']
  ffmpegPath: SettingsModalProps['ffmpegPathValue']
  thumbCacheMb: SettingsModalProps['thumbCacheMbValue']
  mountsPollMs: SettingsModalProps['mountsPollMsValue']
  doubleClickMs: SettingsModalProps['doubleClickMsValue']
  logLevel: SettingsModalProps['logLevelValue']
  scrollbarWidth: SettingsModalProps['scrollbarWidthValue']
  rclonePath: SettingsModalProps['rclonePathValue']
  startDir: SettingsModalProps['startDirValue']
  sortField: SettingsModalProps['sortFieldValue']
  sortDirection: SettingsModalProps['sortDirectionValue']
  shortcuts: SettingsModalProps['shortcuts']

  onChangeDefaultView: SettingsModalProps['onChangeDefaultView']
  onToggleShowHidden: SettingsModalProps['onToggleShowHidden']
  onToggleHiddenFilesLast: SettingsModalProps['onToggleHiddenFilesLast']
  onToggleHighContrast: SettingsModalProps['onToggleHighContrast']
  onToggleFoldersFirst: SettingsModalProps['onToggleFoldersFirst']
  onToggleConfirmDelete: SettingsModalProps['onToggleConfirmDelete']
  onChangeStartDir: SettingsModalProps['onChangeStartDir']
  onChangeDensity: SettingsModalProps['onChangeDensity']
  onChangeArchiveName: SettingsModalProps['onChangeArchiveName']
  onChangeArchiveLevel: SettingsModalProps['onChangeArchiveLevel']
  onToggleOpenDestAfterExtract: SettingsModalProps['onToggleOpenDestAfterExtract']
  onToggleVideoThumbs: SettingsModalProps['onToggleVideoThumbs']
  onToggleHardwareAcceleration: SettingsModalProps['onToggleHardwareAcceleration']
  onChangeFfmpegPath: SettingsModalProps['onChangeFfmpegPath']
  onChangeThumbCacheMb: SettingsModalProps['onChangeThumbCacheMb']
  onChangeMountsPollMs: SettingsModalProps['onChangeMountsPollMs']
  onChangeDoubleClickMs: SettingsModalProps['onChangeDoubleClickMs']
  onChangeLogLevel: SettingsModalProps['onChangeLogLevel']
  onChangeScrollbarWidth: SettingsModalProps['onChangeScrollbarWidth']
  onChangeRclonePath: SettingsModalProps['onChangeRclonePath']
  onClearThumbCache: SettingsModalProps['onClearThumbCache']
  onClearCloudOpenCache: SettingsModalProps['onClearCloudOpenCache']
  onClearStars: SettingsModalProps['onClearStars']
  onClearBookmarks: SettingsModalProps['onClearBookmarks']
  onClearRecents: SettingsModalProps['onClearRecents']
  onChangeSortField: SettingsModalProps['onChangeSortField']
  onChangeSortDirection: SettingsModalProps['onChangeSortDirection']
  onChangeShortcut: SettingsModalProps['onChangeShortcut']
  onClose: SettingsModalProps['onClose']
}

export const createExplorerSettingsModalProps = (p: Params): SettingsModalProps => ({
  open: true,
  initialFilter: p.settingsInitialFilter,
  defaultViewValue: p.defaultViewPref,
  showHiddenValue: p.showHidden,
  hiddenFilesLastValue: p.hiddenFilesLast,
  foldersFirstValue: p.foldersFirst,
  confirmDeleteValue: p.confirmDelete,
  densityValue: p.density,
  highContrastValue: p.highContrast,
  archiveNameValue: p.archiveName,
  archiveLevelValue: p.archiveLevel,
  openDestAfterExtractValue: p.openDestAfterExtract,
  videoThumbsValue: p.videoThumbs,
  hardwareAccelerationValue: p.hardwareAcceleration,
  ffmpegPathValue: p.ffmpegPath,
  thumbCacheMbValue: p.thumbCacheMb,
  mountsPollMsValue: p.mountsPollMs,
  doubleClickMsValue: p.doubleClickMs,
  logLevelValue: p.logLevel,
  scrollbarWidthValue: p.scrollbarWidth,
  rclonePathValue: p.rclonePath,
  startDirValue: p.startDir,
  sortFieldValue: p.sortField,
  sortDirectionValue: p.sortDirection,
  shortcuts: p.shortcuts,
  onChangeDefaultView: p.onChangeDefaultView,
  onToggleShowHidden: p.onToggleShowHidden,
  onToggleHiddenFilesLast: p.onToggleHiddenFilesLast,
  onToggleHighContrast: p.onToggleHighContrast,
  onToggleFoldersFirst: p.onToggleFoldersFirst,
  onToggleConfirmDelete: p.onToggleConfirmDelete,
  onChangeStartDir: p.onChangeStartDir,
  onChangeDensity: p.onChangeDensity,
  onChangeArchiveName: p.onChangeArchiveName,
  onChangeArchiveLevel: p.onChangeArchiveLevel,
  onToggleOpenDestAfterExtract: p.onToggleOpenDestAfterExtract,
  onToggleVideoThumbs: p.onToggleVideoThumbs,
  onToggleHardwareAcceleration: p.onToggleHardwareAcceleration,
  onChangeFfmpegPath: p.onChangeFfmpegPath,
  onChangeThumbCacheMb: p.onChangeThumbCacheMb,
  onChangeMountsPollMs: p.onChangeMountsPollMs,
  onChangeDoubleClickMs: p.onChangeDoubleClickMs,
  onChangeLogLevel: p.onChangeLogLevel,
  onChangeScrollbarWidth: p.onChangeScrollbarWidth,
  onChangeRclonePath: p.onChangeRclonePath,
  onClearThumbCache: p.onClearThumbCache,
  onClearCloudOpenCache: p.onClearCloudOpenCache,
  onClearStars: p.onClearStars,
  onClearBookmarks: p.onClearBookmarks,
  onClearRecents: p.onClearRecents,
  onChangeSortField: p.onChangeSortField,
  onChangeSortDirection: p.onChangeSortDirection,
  onChangeShortcut: p.onChangeShortcut,
  onClose: p.onClose,
})
