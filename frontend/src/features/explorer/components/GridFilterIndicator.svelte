<script lang="ts">
  import type { SortField } from '../model/types'
  import type { ColumnFilters } from '../state/helpers'

  export let columnFilters: ColumnFilters
  export let onResetFilter: (field: SortField) => void = () => {}

  const fields = ['name', 'type', 'modified', 'size'] as const
  $: active = fields.some((field) => columnFilters[field].size > 0)

  const reset = () => {
    for (const field of fields) onResetFilter(field)
  }
</script>

{#if active}
  <div class="grid-filter-indicator">
    <span>Column filters active</span>
    <button type="button" aria-label="Reset column filters" on:click={reset}>Reset</button>
  </div>
{/if}

<style>
  .grid-filter-indicator {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    color: var(--filter-active, var(--accent-error-text));
    font-size: var(--list-header-font-size);
    font-weight: 700;
    letter-spacing: 0.02em;
    padding:
      var(--list-header-padding-y)
      var(--list-header-padding-right)
      var(--list-header-padding-y)
      var(--list-header-padding-left);
    padding-left: calc(var(--list-header-padding-left) + var(--list-rows-padding-left, 15px) - 3px);
  }

  span {
    text-transform: uppercase;
  }

  button {
    padding: 3px 8px;
    border: 1px solid var(--filter-active, var(--accent-error-text));
    border-radius: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    letter-spacing: normal;
    cursor: pointer;
  }

  button:hover {
    background: var(--bg-hover);
  }
</style>
