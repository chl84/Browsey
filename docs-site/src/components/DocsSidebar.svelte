<script lang="ts">
  import type { DocPage } from '../content/pages'
  import { canonicalHash, sectionMatches } from '../lib/docs'

  export let pages: DocPage[] = []
  export let activePageId = ''
  export let searchQuery = ''
  export let compact = false
  export let onSearchChange: (value: string) => void = () => {}
</script>

<aside class="nav">
  <h2>Documentation</h2>

  <label class="searchbox" for="docs-search">
    <span>Search docs</span>
    <input
      id="docs-search"
      type="search"
      placeholder="Search pages and sections"
      value={searchQuery}
      on:input={(event) => onSearchChange((event.currentTarget as HTMLInputElement).value)}
    />
  </label>

  <p class="nav-empty" role="status">
    {#if searchQuery.trim()}
      {pages.length === 0 ? 'No matching pages. Try another search.' : `${pages.length} matching ${pages.length === 1 ? 'page' : 'pages'}.`}
    {/if}
  </p>
  {#if pages.length > 0}
    <details open={!compact || Boolean(searchQuery.trim())}>
      <summary>Browse pages</summary>
      <nav aria-label="Documentation pages">
        {#each pages as page (page.id)}
          <a href={canonicalHash(page.id)} class:active={page.id === activePageId}
            aria-current={page.id === activePageId ? 'page' : undefined}>
            {page.title}
          </a>
          {#if searchQuery.trim()}
            <ul class="search-sections">
              {#each page.sections.filter((section) => sectionMatches(section, searchQuery)) as section (section.id)}
                <li><a href={canonicalHash(page.id, section.id)}>{section.title}</a></li>
              {/each}
            </ul>
          {/if}
        {/each}
      </nav>
    </details>
  {/if}
</aside>
