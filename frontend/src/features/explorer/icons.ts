const ICON_PATHS = [
  'icons/scalable/browsey/shortcut.svg',
  'icons/scalable/browsey/download_folder.svg',
  'icons/scalable/browsey/document_folder.svg',
  'icons/scalable/browsey/pictures_folder.svg',
  'icons/scalable/browsey/video_folder.svg',
  'icons/scalable/browsey/music_folder.svg',
  'icons/scalable/browsey/templates_folder.svg',
  'icons/scalable/browsey/public_folder.svg',
  'icons/scalable/browsey/desktop_folder.svg',
  'icons/scalable/browsey/home.svg',
  'icons/scalable/browsey/folder.svg',
  'icons/scalable/browsey/compressed.svg',
  'icons/scalable/browsey/file.svg',
  'icons/scalable/browsey/textfile.svg',
  'icons/scalable/browsey/picture_file.svg',
  'icons/scalable/browsey/video_file.svg',
  'icons/scalable/browsey/pdf_file.svg',
  'icons/scalable/browsey/spreadsheet_file.svg',
  'icons/scalable/browsey/presentation_file.svg',
] as const

export const iconPath = (id: number | undefined) => ICON_PATHS[id ?? 0] ?? ICON_PATHS[0]

