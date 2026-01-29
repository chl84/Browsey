<script lang="ts">
  import { navIcon } from '../utils'

  export let places: { label: string; path: string }[] = []
  export let onSelect: (label: string, path: string) => void = () => {}
</script>

<div class="section-wrapper">
  <div class="section">
    <div class="section-title">Places</div>
    {#each places as place}
      <button class="nav" type="button" on:click={() => onSelect(place.label, place.path)}>
        <img class="nav-icon" src={navIcon(place.label)} alt="" />
        <span class="nav-label">{place.label}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .section-wrapper {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

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
    border-radius: 0;
    padding: 5px 22px 5px 22px;
    background: transparent;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
  }

  .nav:focus,
  .nav:focus-visible {
    outline: none;
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
