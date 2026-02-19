type TooltipContent = string | (() => string)

const toGetter = (input: TooltipContent): (() => string) =>
  typeof input === 'function' ? (input as () => string) : () => input

export function fullNameTooltip(node: HTMLElement, content: TooltipContent) {
  let getText = toGetter(content)
  let tooltipEl: HTMLDivElement | null = null
  let showTimer: number | null = null

  const place = () => {
    if (!tooltipEl) return
    const rect = node.getBoundingClientRect()
    const ttRect = tooltipEl.getBoundingClientRect()
    const margin = 8
    let left = rect.left + rect.width / 2 - ttRect.width / 2
    left = Math.max(8, Math.min(window.innerWidth - ttRect.width - 8, left))
    const top = rect.top - ttRect.height - margin
    tooltipEl.style.left = `${left}px`
    tooltipEl.style.top = `${Math.max(8, top)}px`
  }

  const show = () => {
    const text = getText()
    if (!text || tooltipEl) return
    tooltipEl = document.createElement('div')
    tooltipEl.className = 'browsey-tooltip'
    tooltipEl.textContent = text
    document.body.appendChild(tooltipEl)
    place()
  }

  const hide = () => {
    if (showTimer !== null) {
      clearTimeout(showTimer)
      showTimer = null
    }
    if (tooltipEl) {
      tooltipEl.remove()
      tooltipEl = null
    }
  }

  const handleMove = () => place()

  const scheduleShow = () => {
    if (tooltipEl || showTimer !== null) return
    showTimer = window.setTimeout(() => {
      showTimer = null
      show()
    }, 750)
  }

  node.addEventListener('mouseenter', scheduleShow)
  node.addEventListener('focus', scheduleShow)
  node.addEventListener('mouseleave', hide)
  node.addEventListener('blur', hide)
  window.addEventListener('scroll', hide, true)
  window.addEventListener('resize', hide)
  node.addEventListener('mousemove', handleMove)

  return {
    update(newContent: TooltipContent) {
      getText = toGetter(newContent)
      if (tooltipEl) {
        const text = getText()
        if (text) {
          tooltipEl.textContent = text
          place()
        } else {
          hide()
        }
      }
    },
    destroy() {
      hide()
      node.removeEventListener('mouseenter', scheduleShow)
      node.removeEventListener('focus', scheduleShow)
      node.removeEventListener('mouseleave', hide)
      node.removeEventListener('blur', hide)
      window.removeEventListener('scroll', hide, true)
      window.removeEventListener('resize', hide)
      node.removeEventListener('mousemove', handleMove)
    },
  }
}
