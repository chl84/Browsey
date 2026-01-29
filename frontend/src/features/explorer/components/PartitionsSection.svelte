<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { partitionIcon } from '../utils'
  import type { Partition } from '../types'

  const dispatch = createEventDispatcher<{ eject: { path: string } }>()

  export let partitions: Partition[] = []
  export let onSelect: (path: string) => void = () => {}

  const eject = (path: string) => dispatch('eject', { path })
</script>

<div class="section">
  <div class="section-title">Partitions</div>
  {#each partitions as part}
    <div class="row">
      <button class="nav" type="button" on:click={() => onSelect(part.path)}>
        <img class="nav-icon" src={partitionIcon(part)} alt="" />
        <span class="nav-label">{part.label}</span>
      </button>
      {#if part.removable}
        <button class="eject" type="button" title="Eject" on:click={() => eject(part.path)}>
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path
              d="M10 4.5 3.5 12h13L10 4.5Zm-6 10h12v2H4v-2Z"
              fill="currentColor"
              fill-rule="evenodd"
              clip-rule="evenodd" />
          </svg>
        </button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-title {
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    padding-left: 10px;
  }

  .row {
    position: relative;
    width: 100%;
  }

  .nav {
    border: none;
    border-radius: 0;
    padding: 5px 40px 5px 22px; /* extra right padding so hover bg reaches behind eject */
    background: transparent;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    width: 100%;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
    text-align: left;
  }

  .nav:hover {
    background: var(--bg-hover);
  }

  .nav:focus,
  .nav:focus-visible {
    outline: none;
  }

  .nav:active {
    transform: none;
    box-shadow: none;
  }

  .nav-icon {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .eject {
    position: absolute;
    top: 50%;
    right: 2px;
    transform: translateY(-50%);
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 4px 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: color 120ms ease;
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
  }

  .eject:hover {
    color: var(--fg);
  }

  .eject svg {
    width: 16px;
    height: 16px;
  }
</style>
