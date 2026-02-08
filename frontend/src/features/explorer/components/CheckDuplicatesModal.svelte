<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import type { Entry } from '../types'

  export let open = false
  export let target: Entry | null = null
  export let searchRoot = ''
  export let duplicates: string[] = []
  export let scanning = false
  export let progressPercent = 0
  export let progressLabel = ''
  export let error = ''
  export let onChangeSearchRoot: (value: string) => void = () => {}
  export let onCopyList: () => void | Promise<void> = () => {}
  export let onSearch: () => void | Promise<void> = () => {}
  export let onClose: () => void = () => {}

  const PREVIEW_LIMIT = 3

  const buildDuplicatePreviewLines = (paths: string[]) => {
    const previewLines = [...paths.slice(0, PREVIEW_LIMIT)]
    const remaining = paths.length - previewLines.length
    if (remaining > 0) {
      previewLines.push(
        `...and ${remaining} additional matching ${remaining === 1 ? 'file' : 'files'}`,
      )
    }
    return previewLines
  }

  $: duplicatePreviewLines = buildDuplicatePreviewLines(duplicates)
  $: hasSummaryLine = duplicates.length > PREVIEW_LIMIT
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
        disabled={scanning}
        on:input={(e) => onChangeSearchRoot((e.currentTarget as HTMLInputElement).value)}
      />
    </label>

    {#if error}
      <div class="pill error">{error}</div>
    {/if}

    {#if scanning || progressLabel}
      <div class="field">
        <span>Progress</span>
        <progress max="100" value={Math.max(0, Math.min(100, progressPercent))}></progress>
        <p class="muted">{progressLabel || `${progressPercent}%`}</p>
      </div>
    {/if}

    <div class="field">
      <div class="field-head">
        <span>Identical files</span>
        {#if duplicates.length > 0}
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
        {/if}
      </div>
      {#if duplicates.length > 0}
        <div class="path duplicate-preview">
          {#each duplicatePreviewLines as line, index}
            <div class="duplicate-preview-line" class:summary={hasSummaryLine && index === duplicatePreviewLines.length - 1}>
              {line}
            </div>
          {/each}
        </div>
      {:else}
        <p class="muted">{scanning ? 'Scanning... symlinks are ignored.' : 'No identical files yet. Symlinks are ignored.'}</p>
      {/if}
    </div>

    <div slot="actions">
      <button type="button" data-initial-focus="1" disabled={scanning} on:click={() => void onSearch()}>
        {scanning ? 'Searching...' : 'Search'}
      </button>
    </div>
  </ModalShell>
{/if}
