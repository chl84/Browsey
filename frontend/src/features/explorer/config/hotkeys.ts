// Central place to list ctrl/cmd-allowed keys to avoid scattering in App.svelte.
// Keys are stored lowercase.
export const allowedCtrlKeys = new Set([
  'f', // search
  'b', // bookmarks modal
  'c', // copy
  'x', // cut
  'v', // paste
  'p', // properties
  'a', // select all
  't', // open console
  'g', // toggle view
  'h', // toggle hidden
  'z', // undo
  'y', // redo
  's', // settings modal
])
