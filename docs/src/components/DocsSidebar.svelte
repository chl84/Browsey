<script lang="ts">
  import type { DocPage } from '../content/pages'

  export let pages: DocPage[] = []
  export let activePageId = ''
  export let searchQuery = ''
  export let onSearchChange: (value: string) => void = () => {}
</script>

<aside class="nav">
  <h2>Documentation</h2>

  <label class="searchbox" for="docs-search">
    <span>Search docs</span>
    <input
      id="docs-search"
      type="search"
      placeholder="Type to filter pages and sections"
      value={searchQuery}
      on:input={(event) => onSearchChange((event.currentTarget as HTMLInputElement).value)}
    />
  </label>

  {#if pages.length === 0}
    <p class="nav-empty">No matching pages.</p>
  {:else}
    <nav aria-label="Documentation pages">
      {#each pages as page (page.id)}
        <a href={`#/${page.id}`} class:active={page.id === activePageId}>
          {page.title}
        </a>
      {/each}
    </nav>
  {/if}
</aside>
