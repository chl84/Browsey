<script lang="ts">
  import type { ShortcutBinding, ShortcutCommandId } from '@/features/shortcuts'

  export let show = false
  export let shortcutColumns: ShortcutBinding[][] = []
  export let shortcutCaptureId: ShortcutCommandId | null = null
  export let shortcutCaptureBusy = false
  export let shortcutCaptureError = ''
  export let onBeginShortcutCapture: (commandId: ShortcutCommandId) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Shortcuts</div><div class="group-spacer"></div>
  <p>Drag &amp; drop: drag local files directly to another app; no Alt key is needed. Hold Ctrl before starting to lock the drag to copy, or Shift to lock it to move. Without these keys the receiving app chooses its default. Incoming drops and internal cloud transfers default to copy. Download cloud files before dragging them to another app.</p>
  <div class="form-control shortcuts-control shortcuts-row">
    <div class="shortcuts-columns">
      {#each shortcutColumns as column, columnIndex (columnIndex)}
        <div class="shortcuts-column">
          {#each column as shortcut (shortcut.commandId)}
            <div class="shortcut-item">
              <span class="shortcut-action">{shortcut.label}</span>
              <button
                type="button"
                class="key shortcut-key"
                class:capturing={shortcutCaptureId === shortcut.commandId}
                disabled={shortcutCaptureBusy && shortcutCaptureId !== shortcut.commandId}
                on:click={() => onBeginShortcutCapture(shortcut.commandId)}
              >
                {#if shortcutCaptureId === shortcut.commandId}
                  {#if shortcutCaptureBusy}
                    Saving...
                  {:else}
                    Press keys
                  {/if}
                {:else}
                  {shortcut.accelerator}
                {/if}
              </button>
            </div>
          {/each}
        </div>
      {/each}
    </div>
    {#if shortcutCaptureError}
      <div class="shortcuts-error">{shortcutCaptureError}</div>
    {/if}
  </div>
{/if}
