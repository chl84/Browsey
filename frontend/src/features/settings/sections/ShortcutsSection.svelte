<script lang="ts">
  import type { ShortcutBinding, ShortcutCommandId } from '../../shortcuts/keymap'

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
