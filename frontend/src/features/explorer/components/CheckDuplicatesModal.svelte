<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import type { Entry } from '../types'

  export let open = false
  export let target: Entry | null = null
  export let searchRoot = ''
  export let duplicates: string[] = []
  export let onChangeSearchRoot: (value: string) => void = () => {}
  export let onCopyList: () => void | Promise<void> = () => {}
  export let onSearch: () => void | Promise<void> = () => {}
  export let onClose: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onClose}
    modalWidth="560px"
    initialFocusSelector="button[data-initial-focus='1']"
  >
    <svelte:fragment slot="header">Check for Duplicates</svelte:fragment>

    {#if target}
      <div class="field">
        <span>Selected file</span>
        <div class="path">{target.path}</div>
      </div>
    {/if}

    <label class="field" for="duplicates-start-root">
      <span>Start folder</span>
      <input
        id="duplicates-start-root"
        type="text"
        autocomplete="off"
        value={searchRoot}
        on:input={(e) => onChangeSearchRoot((e.currentTarget as HTMLInputElement).value)}
      />
    </label>

    <div class="field">
      <span>Identical files</span>
      {#if duplicates.length > 0}
        <div class="actions">
          <button
            type="button"
            class="secondary icon-btn"
            aria-label="Copy duplicates list"
            title="Copy list"
            on:click={() => void onCopyList()}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <rect x="9" y="9" width="13" height="13"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          </button>
        </div>
        <pre class="path">{duplicates.join('\n')}</pre>
      {:else}
        <p class="muted">No identical files yet. Symlinks are ignored.</p>
      {/if}
    </div>

    <div slot="actions">
      <button type="button" data-initial-focus="1" on:click={() => void onSearch()}>Search</button>
    </div>
  </ModalShell>
{/if}
