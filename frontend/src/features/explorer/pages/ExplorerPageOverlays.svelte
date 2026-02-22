<script lang="ts">
  import type { ShortcutBinding } from '@/features/shortcuts'
  import AboutBrowseyModal from '@/features/explorer/components/AboutBrowseyModal.svelte'
  import TextContextMenu from '@/features/explorer/components/TextContextMenu.svelte'
  import ConflictModal from '@/shared/ui/ConflictModal.svelte'
  import DragGhost from '@/shared/ui/DragGhost.svelte'

  type ConflictEntry = { src: string; target: string; is_dir: boolean }
  type DragAction = 'copy' | 'move' | null

  export let dragGhostVisible = false
  export let dragGhostX = 0
  export let dragGhostY = 0
  export let dragGhostCount = 0
  export let dragGhostAllowed = true
  export let dragGhostAction: DragAction = null

  export let textMenuOpen = false
  export let textMenuX = 0
  export let textMenuY = 0
  export let textMenuTarget: HTMLElement | null = null
  export let textMenuReadonly = false
  export let shortcutBindings: ShortcutBinding[] = []
  export let closeTextContextMenu: () => void = () => {}

  export let conflictModalOpen = false
  export let conflictList: ConflictEntry[] = []
  export let cancelConflicts: () => void = () => {}
  export let renameAllConflicts: () => void = () => {}
  export let overwriteConflicts: () => void = () => {}

  export let aboutOpen = false
  export let closeAbout: () => void = () => {}
</script>

<DragGhost
  visible={dragGhostVisible}
  x={dragGhostX}
  y={dragGhostY}
  count={dragGhostCount}
  allowed={dragGhostAllowed}
  action={dragGhostAction}
/>
<TextContextMenu
  open={textMenuOpen}
  x={textMenuX}
  y={textMenuY}
  target={textMenuTarget}
  readonly={textMenuReadonly}
  shortcuts={shortcutBindings}
  onClose={closeTextContextMenu}
/>
<slot />
<ConflictModal
  open={conflictModalOpen}
  conflicts={conflictList}
  onCancel={cancelConflicts}
  onRenameAll={renameAllConflicts}
  onOverwrite={overwriteConflicts}
/>
<AboutBrowseyModal open={aboutOpen} onClose={closeAbout} />

