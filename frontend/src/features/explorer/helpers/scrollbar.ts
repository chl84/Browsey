export const isScrollbarClick = (event: MouseEvent, el: HTMLDivElement | null) => {
  if (!el) return false
  const rect = el.getBoundingClientRect()
  const scrollbarX = el.offsetWidth - el.clientWidth
  const scrollbarY = el.offsetHeight - el.clientHeight
  if (scrollbarX > 0) {
    const x = event.clientX - rect.left
    if (x >= el.clientWidth) return true
  }
  if (scrollbarY > 0) {
    const y = event.clientY - rect.top
    if (y >= el.clientHeight) return true
  }
  return false
}
