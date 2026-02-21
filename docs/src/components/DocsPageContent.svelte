<script lang="ts">
  import type { DocPage } from '../content/pages'

  export let page: DocPage
  export let activeSectionId = ''

  const sectionAnchorId = (pageId: string, sectionId: string) => `${pageId}--${sectionId}`
  const sectionHash = (pageId: string, sectionId: string) =>
    `#/${encodeURIComponent(pageId)}/${encodeURIComponent(sectionId)}`
</script>

<section class="content" aria-label={`${page.title} content`}>
  {#each page.sections as section (section.id)}
    <article
      id={sectionAnchorId(page.id, section.id)}
      class="card"
      class:active-section={section.id === activeSectionId}
    >
      <h2>
        <a href={sectionHash(page.id, section.id)} class="section-anchor">{section.title}</a>
      </h2>
      {#if section.body}
        <p>{section.body}</p>
      {/if}
      {#if section.bullets && section.bullets.length > 0}
        <ul>
          {#each section.bullets as bullet, bulletIndex (`${section.id}-${bulletIndex}-${bullet}`)}
            <li>{bullet}</li>
          {/each}
        </ul>
      {/if}
      {#if section.code}
        <pre><code>{section.code}</code></pre>
      {/if}
      {#if section.note}
        <p class="note">{section.note}</p>
      {/if}
    </article>
  {/each}
</section>
