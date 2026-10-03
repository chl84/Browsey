<script lang="ts">
  import { onMount, tick } from 'svelte'
  import DocsOnThisPage from './components/DocsOnThisPage.svelte'
  import DocsPageContent from './components/DocsPageContent.svelte'
  import DocsSidebar from './components/DocsSidebar.svelte'
  import { docsPages, docsPageMap, type DocPage } from './content/pages'
  import { canonicalHash, filterPages, normalizeHash, sectionAnchorId } from './lib/docs'

  const fallbackPageId = docsPages[0].id

  let activePageId = fallbackPageId
  let activeSectionId = ''
  let activePage: DocPage = docsPageMap.get(fallbackPageId)!
  let searchQuery = ''
  let compactNavigation = false
  let compactTableOfContents = false
  $: filteredPages = filterPages(docsPages, searchQuery)
  let navigationRevision = 0

  const syncFromHash = async (focus = false) => {
    const revision = ++navigationRevision
    const { pageId, sectionId } = normalizeHash(window.location.hash, docsPages)
    activePageId = pageId
    activeSectionId = sectionId
    activePage = docsPageMap.get(activePageId) ?? docsPageMap.get(fallbackPageId)!

    const hash = canonicalHash(activePageId, activeSectionId)
    if (window.location.hash !== hash) {
      window.history.replaceState(null, '', hash)
    }

    document.title = `${activePage.title} — Browsey Documentation`
    document.querySelector('meta[name="description"]')?.setAttribute('content', activePage.summary)
    await tick()
    if (revision !== navigationRevision) return
    const target = document.getElementById(sectionId ? sectionAnchorId(pageId, sectionId) : 'docs-content')
    if (sectionId) target?.scrollIntoView({ block: 'start', behavior: 'instant' })
    else window.scrollTo({ top: 0, behavior: 'instant' })
    if (focus) target?.focus({ preventScroll: true })
  }

  const handleSearchChange = (value: string) => {
    searchQuery = value
  }

  const skipToContent = (event: MouseEvent) => {
    event.preventDefault()
    const content = document.getElementById('docs-content')
    content?.focus()
    content?.scrollIntoView({ block: 'start' })
  }

  onMount(() => {
    const compactMedia = window.matchMedia('(max-width: 900px)')
    const tocMedia = window.matchMedia('(max-width: 1100px)')
    const updateNavigationLayout = () => {
      compactNavigation = compactMedia.matches
      compactTableOfContents = tocMedia.matches
    }
    updateNavigationLayout()
    compactMedia.addEventListener('change', updateNavigationLayout)
    tocMedia.addEventListener('change', updateNavigationLayout)
    const handleHashChange = () => {
      void syncFromHash(true)
    }
    let scrollFrame: number | null = null
    const handleScroll = () => {
      if (scrollFrame !== null) return
      scrollFrame = requestAnimationFrame(() => {
        scrollFrame = null
        const sections = activePage.sections
        const current = sections.filter((section) => {
          const top = document.getElementById(sectionAnchorId(activePageId, section.id))?.getBoundingClientRect().top
          return top !== undefined && top <= 120
        }).at(-1)
        const atBottom = window.scrollY + window.innerHeight >= document.documentElement.scrollHeight - 2
        activeSectionId = (atBottom ? sections.at(-1) : current)?.id ?? ''
      })
    }
    void syncFromHash()
    window.addEventListener('hashchange', handleHashChange)
    window.addEventListener('scroll', handleScroll, { passive: true })

    return () => {
      window.removeEventListener('hashchange', handleHashChange)
      window.removeEventListener('scroll', handleScroll)
      compactMedia.removeEventListener('change', updateNavigationLayout)
      tocMedia.removeEventListener('change', updateNavigationLayout)
      if (scrollFrame !== null) cancelAnimationFrame(scrollFrame)
    }
  })

</script>

<a class="skip-link" href="#docs-content" on:click={skipToContent}>Skip to content</a>
<main class="shell">
  <header class="top">
    <p class="eyebrow">Browsey Documentation</p>
    <h1>{activePage.title}</h1>
    <p class="lede">{activePage.summary}</p>
    <nav class="project-links" aria-label="Browsey project">
      <a href="https://github.com/chl84/Browsey/releases/latest">Download Browsey</a>
      <a href={canonicalHash('getting-started')}>Install</a>
      <a href="https://github.com/chl84/Browsey">GitHub</a>
    </nav>
  </header>

  <div class="layout">
    <DocsSidebar
      pages={filteredPages}
      activePageId={activePageId}
      searchQuery={searchQuery}
      onSearchChange={handleSearchChange}
      compact={compactNavigation}
    />

    <DocsPageContent page={activePage} activeSectionId={activeSectionId} />

    <DocsOnThisPage page={activePage} activeSectionId={activeSectionId} compact={compactTableOfContents} />
  </div>
</main>
