<script lang="ts">
  import type { DataClearTarget } from '../settingsTypes'

  export let show = false
  export let clearBusy = false
  export let clearTarget: DataClearTarget | null = null
  export let onRequestClear: (target: DataClearTarget) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Data</div><div class="group-spacer"></div>

  <div class="form-label">Clear thumbnail cache</div>
  <div class="form-control">
    <button
      type="button"
      class="secondary"
      disabled={clearBusy}
      on:click={() => onRequestClear('thumb-cache')}
    >
      {clearBusy && clearTarget === 'thumb-cache' ? 'Clearing...' : 'Clear'}
    </button>
  </div>

  <div class="form-label">Clear stars</div>
  <div class="form-control">
    <button type="button" class="secondary" disabled={clearBusy} on:click={() => onRequestClear('stars')}>
      {clearBusy && clearTarget === 'stars' ? 'Clearing...' : 'Clear'}
    </button>
  </div>

  <div class="form-label">Clear bookmarks</div>
  <div class="form-control">
    <button
      type="button"
      class="secondary"
      disabled={clearBusy}
      on:click={() => onRequestClear('bookmarks')}
    >
      {clearBusy && clearTarget === 'bookmarks' ? 'Clearing...' : 'Clear'}
    </button>
  </div>

  <div class="form-label">Clear recents</div>
  <div class="form-control">
    <button type="button" class="secondary" disabled={clearBusy} on:click={() => onRequestClear('recents')}>
      {clearBusy && clearTarget === 'recents' ? 'Clearing...' : 'Clear'}
    </button>
  </div>
{/if}
