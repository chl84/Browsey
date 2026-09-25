// WebKitGTK sanitizes a multiline text/uri-list as ONE URL, removing CR/LF.
// A single encoded envelope survives that sanitization; our GTK data-get hook
// replaces it with real, individually escaped file URIs before native delivery.
export const fileDragPayload = (paths: string[], nativeBridge: boolean): string | null => {
  if (!paths.length || paths.some(path => path.startsWith('rclone://'))) return null
  if (nativeBridge) {
    if (paths.some(path => !path.startsWith('/') || path.includes('\0'))) return null
    const envelope = new URL('browsey-drag://local/')
    envelope.searchParams.set('paths', JSON.stringify(paths))
    return envelope.href
  }
  // Other webviews receive conventional URI lists. Linux uses the bridge above.
  const uris = paths.map(path => {
    const normalized = /^[a-z]:\\/i.test(path) || path.startsWith('\\\\') ? path.replace(/\\/g, '/') : path
    if (!normalized.startsWith('/') && !/^[a-z]:\//i.test(normalized)) return null
    if (normalized.includes('\0')) return null
    const encoded = normalized.split('/').map(segment => encodeURIComponent(segment)).join('/')
    return normalized.startsWith('//') ? `file:${encoded}`
      : normalized.startsWith('/') ? `file://${encoded}`
        : `file:///${encoded.replace(/^([a-z])%3A/i, '$1:')}`
  })
  return uris.every((uri): uri is string => uri !== null) ? `${uris.join('\r\n')}\r\n` : null
}

export const hasNativeFileDragBridge = () =>
  (window as unknown as { __BROWSEY_FILE_DRAG_BRIDGE__?: boolean }).__BROWSEY_FILE_DRAG_BRIDGE__ === true

// GTK/Wayland receivers do not consistently honour modifiers changed during a
// cross-process drag. A modifier held at start narrows the offered action set.
export const fileDragStartMode = (event: Pick<DragEvent, 'ctrlKey' | 'metaKey' | 'shiftKey'>) =>
  event.ctrlKey || event.metaKey ? 'copy' : event.shiftKey ? 'cut' : null
