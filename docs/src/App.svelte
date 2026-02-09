<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { docsPages, docsPageMap, type DocPage } from './content/pages'

  const fallbackPageId = docsPages[0].id
  let activePageId = fallbackPageId
  let activePage: DocPage = docsPageMap.get(fallbackPageId)!

  const normalizeHash = () => {
    const raw = window.location.hash.trim()
    if (!raw || raw === '#') return fallbackPageId
    const noPound = raw.startsWith('#') ? raw.slice(1) : raw
    const noSlash = noPound.startsWith('/') ? noPound.slice(1) : noPound
    const pageId = decodeURIComponent(noSlash)
    return docsPageMap.has(pageId) ? pageId : fallbackPageId
  }

  const syncFromHash = () => {
    activePageId = normalizeHash()
    activePage = docsPageMap.get(activePageId) ?? docsPageMap.get(fallbackPageId)!
    if (window.location.hash !== `#/${activePageId}`) {
      window.history.replaceState(null, '', `#/${activePageId}`)
    }
  }

  onMount(() => {
    syncFromHash()
    window.addEventListener('hashchange', syncFromHash)
  })

  onDestroy(() => {
    window.removeEventListener('hashchange', syncFromHash)
  })
</script>

<main class="shell">
  <header class="top">
    <p class="eyebrow">Browsey Documentation</p>
    <h1>{activePage.title}</h1>
    <p class="lede">{activePage.summary}</p>
  </header>

  <div class="layout">
    <aside class="nav">
      <h2>Documentation</h2>
      <nav aria-label="Documentation pages">
        {#each docsPages as page}
          <a href={`#/${page.id}`} class:active={page.id === activePageId}>
            {page.title}
          </a>
        {/each}
      </nav>
    </aside>

    <section class="content">
      {#each activePage.sections as section}
        <article id={section.id} class="card">
          <h2>{section.title}</h2>
          {#if section.body}
            <p>{section.body}</p>
          {/if}
          {#if section.bullets && section.bullets.length > 0}
            <ul>
              {#each section.bullets as bullet}
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
  </div>
</main>
