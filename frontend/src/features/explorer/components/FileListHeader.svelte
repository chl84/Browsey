<script lang="ts">
  import ColumnResizer from '../../../ui/ColumnResizer.svelte'
  import type { Column, SortDirection, SortField } from '../types'

  export let cols: Column[] = []
  export let gridTemplate = ''
  export let headerEl: HTMLDivElement | null = null
  export let sortField: SortField = 'name'
  export let sortDirection: SortDirection = 'asc'
  export let ariaSort: (field: SortField) => 'ascending' | 'descending' | 'none' = () => 'none'
  export let onChangeSort: (field: SortField) => void = () => {}
  export let onStartResize: (index: number, event: PointerEvent) => void = () => {}
</script>

<div class="header-row" bind:this={headerEl} style={`grid-template-columns:${gridTemplate};`}>
  {#each cols as col, idx}
    <div class="header-cell">
      {#if col.sortable === false}
        <div
          class="header-btn inert"
          class:align-right={col.align === 'right'}
          role="columnheader"
          aria-sort="none"
        >
          {#if col.label}<span>{col.label}</span>{/if}
        </div>
      {:else}
        <button
          class="header-btn"
          class:align-right={col.align === 'right'}
          type="button"
          role="columnheader"
          tabindex="-1"
          aria-sort={ariaSort(col.sort)}
          class:active-sort={sortField === col.sort}
          on:click={() => onChangeSort(col.sort)}
        >
          <span>{col.label}</span>
          <span
            class="sort-icon"
            class:desc={sortField === col.sort && sortDirection === 'desc'}
            class:inactive={sortField !== col.sort}
          >
            ▲
          </span>
        </button>
      {/if}
      {#if col.resizable !== false && idx < cols.length - 1}
        <ColumnResizer onStart={(e) => onStartResize(idx, e)} />
      {/if}
    </div>
  {/each}
</div>

<style>
  .header-row {
    display: grid;
    gap: var(--list-header-gap);
    padding:
      var(--list-header-padding-y)
      var(--list-header-padding-right)
      var(--list-header-padding-y)
      var(--list-header-padding-left);
    border-bottom: 1px solid var(--border-strong);
    background: var(--bg);
    color: var(--fg-muted);
    font-size: var(--list-header-font-size);
    letter-spacing: 0.02em;
    text-transform: uppercase;
    position: sticky;
    top: 0;
    z-index: 1;
    width: max-content;
  }

  .header-cell {
    display: flex;
    align-items: center;
    position: relative;
    gap: var(--list-header-cell-gap);
    min-width: 0;
    flex: 1 1 0;
  }

  .header-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--list-header-cell-gap);
    justify-content: flex-start;
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: inherit;
    font-size: var(--list-header-font-size);
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 0;
    text-align: left;
  }

  .header-btn.align-right {
    justify-content: flex-end;
    text-align: right;
    margin-right: calc(-1 * var(--list-header-align-right-offset));
    padding-right: var(--list-header-align-right-offset);
  }

  .header-btn.inert {
    cursor: default;
    pointer-events: none;
  }

  .header-btn.active-sort {
    color: var(--fg);
  }

  .header-btn:focus-visible {
    outline: 2px solid var(--border-accent);
    border-radius: 0;
    outline-offset: 2px;
  }

  .sort-icon {
    font-size: var(--list-header-sort-icon-size);
    opacity: 0.8;
    display: inline-flex;
    align-items: center;
    transition: transform 120ms ease;
  }

  .sort-icon.inactive {
    opacity: 0.35;
  }

  .sort-icon.desc {
    transform: rotate(180deg);
  }
</style>
