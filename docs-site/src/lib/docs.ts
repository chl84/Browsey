import type { DocPage, DocSection } from '../content/pages'

export const sectionAnchorId = (pageId: string, sectionId: string) => `${pageId}--${sectionId}`
export const canonicalHash = (pageId: string, sectionId = '') =>
  `#/${encodeURIComponent(pageId)}${sectionId ? `/${encodeURIComponent(sectionId)}` : ''}`

export function normalizeHash(hash: string, pages: DocPage[]) {
  const decode = (part: string) => {
    try { return decodeURIComponent(part) } catch { return part }
  }
  const [rawPage = '', rawSection = ''] = hash.trim().replace(/^#\/?/, '').split('/')
  const page = pages.find((item) => item.id === decode(rawPage)) ?? pages[0]
  const sectionId = decode(rawSection)
  return {
    pageId: page.id,
    sectionId: page.sections.some((section) => section.id === sectionId) ? sectionId : '',
  }
}

const matches = (text: string, query: string) =>
  query.trim().toLowerCase().split(/\s+/).every((term) => text.toLowerCase().includes(term))

export function sectionMatches(section: DocSection, query: string) {
  return matches([
    section.title, section.body, section.note, section.code,
    ...(section.bullets ?? []), ...(section.links ?? []).map((link) => link.label),
  ].join(' '), query)
}

export function filterPages(pages: DocPage[], query: string) {
  return pages.filter((page) => matches([
    page.title, page.summary,
    ...page.sections.flatMap((section) => [
      section.title, section.body, section.note, section.code,
      ...(section.bullets ?? []), ...(section.links ?? []).map((link) => link.label),
    ]),
  ].join(' '), query))
}

// Only inline backtick code is supported. Svelte escapes all remaining text;
// documentation is never interpreted as arbitrary HTML.
export function inlineParts(text: string) {
  return text.split(/(`[^`]+`)/g).filter(Boolean).map((value) => ({
    code: value.startsWith('`') && value.endsWith('`') && value.length > 2,
    text: value.startsWith('`') && value.endsWith('`') && value.length > 2
      ? value.slice(1, -1) : value,
  }))
}
