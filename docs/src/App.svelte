<script lang="ts">
  import { onMount, tick } from 'svelte'
  import DocsOnThisPage from './components/DocsOnThisPage.svelte'
  import DocsPageContent from './components/DocsPageContent.svelte'
  import DocsSidebar from './components/DocsSidebar.svelte'
  import { docsPages, docsPageMap, type DocPage, type DocSection } from './content/pages'

  const fallbackPageId = docsPages[0].id
  const sectionAnchorId = (pageId: string, sectionId: string) => `${pageId}--${sectionId}`
  const canonicalHash = (pageId: string, sectionId = '') =>
    sectionId
      ? `#/${encodeURIComponent(pageId)}/${encodeURIComponent(sectionId)}`
      : `#/${encodeURIComponent(pageId)}`

  let activePageId = fallbackPageId
  let activeSectionId = ''
  let activePage: DocPage = docsPageMap.get(fallbackPageId)!
  let searchQuery = ''
  let filteredPages: DocPage[] = docsPages

  const decodeHashPart = (value: string) => {
    try {
      return decodeURIComponent(value)
    } catch {
      return value
    }
  }

  const normalizeHash = () => {
    const raw = window.location.hash.trim()
    if (!raw || raw === '#') {
      return { pageId: fallbackPageId, sectionId: '' }
    }

    const withoutPound = raw.startsWith('#') ? raw.slice(1) : raw
    const withoutSlash = withoutPound.startsWith('/') ? withoutPound.slice(1) : withoutPound
    const [rawPageId = '', rawSectionId = ''] = withoutSlash.split('/')

    const pageIdCandidate = decodeHashPart(rawPageId)
    const pageId = docsPageMap.has(pageIdCandidate) ? pageIdCandidate : fallbackPageId
    const page = docsPageMap.get(pageId) ?? docsPageMap.get(fallbackPageId)!

    const sectionIdCandidate = decodeHashPart(rawSectionId)
    const sectionId = page.sections.some((section) => section.id === sectionIdCandidate)
      ? sectionIdCandidate
      : ''

    return { pageId, sectionId }
  }

  const scrollToSection = async (pageId: string, sectionId: string) => {
    if (!sectionId) return
    await tick()
    const target = document.getElementById(sectionAnchorId(pageId, sectionId))
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' })
    }
  }

  const syncFromHash = async () => {
    const { pageId, sectionId } = normalizeHash()
    activePageId = pageId
    activeSectionId = sectionId
    activePage = docsPageMap.get(activePageId) ?? docsPageMap.get(fallbackPageId)!

    const hash = canonicalHash(activePageId, activeSectionId)
    if (window.location.hash !== hash) {
      window.history.replaceState(null, '', hash)
    }

    await scrollToSection(activePageId, activeSectionId)
  }

  const sectionMatches = (section: DocSection, needle: string) => {
    const terms = [section.title, section.body, section.note, section.code, ...(section.bullets ?? [])]
    return terms.some((term) => term?.toLowerCase().includes(needle))
  }

  const applySearchFilter = () => {
    const needle = searchQuery.trim().toLowerCase()
    if (!needle) {
      filteredPages = docsPages
      return
    }

    filteredPages = docsPages.filter((page) => {
      const pageTerms = [page.title, page.summary]
      const pageMatch = pageTerms.some((term) => term.toLowerCase().includes(needle))
      return pageMatch || page.sections.some((section) => sectionMatches(section, needle))
    })
  }

  const handleSearchChange = (value: string) => {
    searchQuery = value
    applySearchFilter()

    if (filteredPages.length === 0 || filteredPages.some((page) => page.id === activePageId)) {
      return
    }

    window.location.hash = canonicalHash(filteredPages[0].id)
  }

  onMount(() => {
    const handleHashChange = () => {
      void syncFromHash()
    }

    applySearchFilter()
    void syncFromHash()
    window.addEventListener('hashchange', handleHashChange)

    return () => {
      window.removeEventListener('hashchange', handleHashChange)
    }
  })

</script>

<main class="shell">
  <header class="top">
    <p class="eyebrow">Browsey Documentation</p>
    <h1>{activePage.title}</h1>
    <p class="lede">{activePage.summary}</p>
  </header>

  <div class="layout">
    <DocsSidebar
      pages={filteredPages}
      activePageId={activePageId}
      searchQuery={searchQuery}
      onSearchChange={handleSearchChange}
    />

    <DocsPageContent page={activePage} activeSectionId={activeSectionId} />

    <DocsOnThisPage page={activePage} activeSectionId={activeSectionId} />
  </div>
</main>
