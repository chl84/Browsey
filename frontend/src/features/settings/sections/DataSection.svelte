<script lang="ts">
  import type { DataClearTarget } from '../settingsTypes'
  import UndoStorageSection from './UndoStorageSection.svelte'

  export let show = false
  export let clearBusy = false
  export let clearTarget: DataClearTarget | null = null
  export let onRequestClear: (target: DataClearTarget) => void = () => {}

  const groups = [
    { label: 'Caches', actions: [
      { target: 'thumb-cache', label: 'Clear thumbnail cache' },
      { target: 'cloud-open-cache', label: 'Clear cloud file cache' },
    ] },
    { label: 'Saved lists', actions: [
      { target: 'stars', label: 'Clear stars' },
      { target: 'bookmarks', label: 'Clear bookmarks' },
      { target: 'recents', label: 'Clear recents' },
    ] },
  ] satisfies { label: string; actions: { target: DataClearTarget; label: string }[] }[]
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Stored data</div><div class="group-spacer"></div>

  <UndoStorageSection />

  {#each groups as group (group.label)}
    <div class="form-label data-label">{group.label}</div>
    <div class="form-control data-actions" role="group" aria-label={group.label}>
      {#each group.actions as action (action.target)}
        <button
          type="button"
          class="secondary"
          aria-label={action.label}
          disabled={clearBusy}
          on:click={() => onRequestClear(action.target)}
        >
          {clearBusy && clearTarget === action.target ? 'Clearing…' : action.label}
        </button>
      {/each}
    </div>
  {/each}
{/if}

<style>
  .data-actions {
    flex-wrap: wrap;
    min-width: 0;
  }
  .data-label {
    align-self: start;
    padding-top: var(--settings-control-gap);
  }
</style>
