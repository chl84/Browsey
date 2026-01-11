<script lang="ts">
  import { partitionIcon } from '../../explorer/utils'
  import type { Partition } from '../../explorer/types'

  export let partitions: Partition[] = []
  export let onSelect: (path: string) => void = () => {}
</script>

<div class="section">
  <div class="section-title">Partitions</div>
  {#each partitions as part}
    <button class="nav" type="button" on:click={() => onSelect(part.path)}>
      <img class="nav-icon" src={partitionIcon(part)} alt="" />
      <span class="nav-label">{part.label}</span>
    </button>
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

  .nav {
    border: none;
    border-radius: 10px;
    padding: 5px 12px 5px 22px;
    background: transparent;
    color: var(--fg);
    font-size: 14px;
    font-weight: 500;
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
  }

  .nav:hover {
    background: var(--bg-hover);
    transform: none;
    box-shadow: none;
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
</style>
