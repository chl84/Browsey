// Run through the collaborative preview in Vite's e2e mode, never a native account.
type Call = { cmd: string; args?: Record<string, unknown> }
type Control = {
  thumbnailFixture: boolean
  performanceFixture: { entries: number; thumbnailDelayMs?: number }
  calls: Call[]
}
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))
const wait = async (predicate: () => boolean) => {
  const deadline = performance.now() + 15_000
  while (!predicate()) {
    if (performance.now() > deadline) throw new Error('Performance fixture did not reach its expected state')
    await pause(10)
  }
}
const viewport = () => {
  const node = document.querySelector<HTMLElement>('.rows, .grid')
  if (!node) throw new Error('File viewport is missing; close dialogs and open /mock first')
  return node
}
const wheel = (deltaY: number) => {
  const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaY })
  viewport().dispatchEvent(event)
  if (!event.defaultPrevented) throw new Error('Zoom event was not accepted')
}
const stats = (samples: number[]) => {
  const sorted = [...samples].sort((a, b) => a - b)
  return { samplesMs: samples, medianMs: sorted[Math.floor(sorted.length / 2)], maxMs: sorted.at(-1) }
}

export const measureExplorerUi = async (entries: number) => {
  if (import.meta.env.MODE !== 'e2e' || ![10_000, 100_000].includes(entries)) {
    throw new Error('Use the disposable e2e server and a 10k or 100k fixture')
  }
  const host = window as unknown as { __BROWSEY_E2E__: Control }
  const control: Control = { thumbnailFixture: true, performanceFixture: { entries }, calls: [] }
  host.__BROWSEY_E2E__ = control
  // Reset to list through the real wheel handler, independently of saved settings.
  for (let step = 0; step < 6 && document.querySelector('.grid'); step++) {
    wheel(120)
    await pause(100)
  }
  if (!document.querySelector('.rows')) throw new Error('List mode reset failed')
  const refresh: number[] = []
  for (let sample = 0; sample < 6; sample++) {
    viewport().scrollTop = 0
    viewport().focus()
    const calls = control.calls.length
    const start = performance.now()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'F5', code: 'F5', bubbles: true, cancelable: true }))
    await wait(() => control.calls.slice(calls).some(call => call.cmd === 'list_dir'))
    await frame()
    await frame()
    if (viewport().scrollHeight < entries * 25) throw new Error('Large listing was not rendered')
    refresh.push(performance.now() - start)
  }
  const firstThumbStart = performance.now()
  wheel(-120)
  await wait(() => !!document.querySelector('.card[data-path="/mock/photo-000.jpg"] img.icon[src^="data:image"]'))
  await frame()
  const firstThumbMs = performance.now() - firstThumbStart
  await pause(100)
  const scroll: number[] = []
  const zoom: number[] = []
  const domCounts: number[] = []
  for (let sample = 0; sample < 5; sample++) {
    const node = viewport()
    const last = `/mock/photo-${String(entries - 1).padStart(3, '0')}.jpg`
    const start = performance.now()
    node.scrollTop = node.scrollHeight
    await wait(() => !!document.querySelector(`.card[data-path="${last}"]`))
    await frame()
    scroll.push(performance.now() - start)
    domCounts.push(document.querySelectorAll('.card').length)
    const zoomStart = performance.now()
    wheel(sample % 2 === 0 ? -120 : 120)
    await frame()
    await frame()
    zoom.push(performance.now() - zoomStart)
    await pause(100)
    viewport().scrollTop = 0
    await frame()
    await frame()
  }
  const memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory
  return {
    schema: 1, entries, viewport: { width: innerWidth, height: innerHeight, ratio: devicePixelRatio },
    firstRefreshMs: refresh[0], refresh: stats(refresh.slice(1)), firstMockThumbMs: firstThumbMs,
    scroll: stats(scroll), zoom: stats(zoom), renderedCards: domCounts,
    usedJsHeapBytes: memory?.usedJSHeapSize ?? null,
    thumbnailRequests: control.calls.filter(call => call.cmd === 'get_thumbnail').length,
    scope: 'development browser UI with mock IPC and synthetic images; two animation-frame readiness boundary, not native WebKit or real I/O',
  }
}
