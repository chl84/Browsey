<script lang="ts">
  import type { DocPage } from '../content/pages'
  import { canonicalHash } from '../lib/docs'

  export let page: DocPage
  export let activeSectionId = ''
  export let compact = false

</script>

<aside class="toc" aria-label="On this page">
  <details open={!compact}>
    <summary>On this page</summary>
    <nav aria-label="Sections on this page">
      {#each page.sections as section (section.id)}
        <a href={canonicalHash(page.id, section.id)} class:active={section.id === activeSectionId}
          aria-current={section.id === activeSectionId ? 'location' : undefined}>
          {section.title}
        </a>
      {/each}
    </nav>
  </details>
</aside>
