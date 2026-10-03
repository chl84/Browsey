<script lang="ts">
  import type { DocPage } from '../content/pages'
  import DocsText from './DocsText.svelte'
  import { canonicalHash, sectionAnchorId } from '../lib/docs'

  export let page: DocPage
  export let activeSectionId = ''

</script>

<section id="docs-content" class="content" tabindex="-1" aria-label={`${page.title} content`}>
  {#each page.sections as section (section.id)}
    <article
      id={sectionAnchorId(page.id, section.id)}
      class="card"
      class:active-section={section.id === activeSectionId}
      tabindex="-1"
    >
      <h2>
        <a href={canonicalHash(page.id, section.id)} class="section-anchor">{section.title}</a>
      </h2>
      {#if section.body}
        <p><DocsText text={section.body} /></p>
      {/if}
      {#if section.bullets && section.bullets.length > 0}
        <ul>
          {#each section.bullets as bullet, bulletIndex (`${section.id}-${bulletIndex}-${bullet}`)}
            <li><DocsText text={bullet} /></li>
          {/each}
        </ul>
      {/if}
      {#if section.code}
        <pre><code>{section.code}</code></pre>
      {/if}
      {#if section.note}
        <p class="note"><DocsText text={section.note} /></p>
      {/if}
      {#if section.links?.length}
        <ul class="related-links" aria-label={`${section.title} links`}>
          {#each section.links as link (link.href)}
            <li><a href={link.href}>{link.label}</a></li>
          {/each}
        </ul>
      {/if}
    </article>
  {/each}
</section>
