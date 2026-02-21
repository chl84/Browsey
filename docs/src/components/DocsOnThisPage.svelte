<script lang="ts">
  import type { DocPage } from '../content/pages'

  export let page: DocPage
  export let activeSectionId = ''

  const sectionHash = (pageId: string, sectionId: string) =>
    `#/${encodeURIComponent(pageId)}/${encodeURIComponent(sectionId)}`
</script>

<aside class="toc" aria-label="On this page">
  <h2>On this page</h2>
  <nav>
    {#each page.sections as section (section.id)}
      <a href={sectionHash(page.id, section.id)} class:active={section.id === activeSectionId}>
        {section.title}
      </a>
    {/each}
  </nav>
</aside>
