export type TooltipContent = string | (() => string)

type TooltipInput =
  | TooltipContent
  | {
      content: TooltipContent
      delayMs?: number
    }

const DEFAULT_DELAY_MS = 750

const toGetter = (input: TooltipContent): (() => string) =>
  typeof input === 'function' ? (input as () => string) : () => input

const parseInput = (input: TooltipInput): { getText: () => string; delayMs: number } => {
  if (typeof input === 'string' || typeof input === 'function') {
    return { getText: toGetter(input), delayMs: DEFAULT_DELAY_MS }
  }
  return {
    getText: toGetter(input.content),
    delayMs: input.delayMs ?? DEFAULT_DELAY_MS,
  }
}

export function tooltip(node: HTMLElement, input: TooltipInput) {
  let { getText, delayMs } = parseInput(input)
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
    }, delayMs)
  }

  node.addEventListener('mouseenter', scheduleShow)
  node.addEventListener('focus', scheduleShow)
  node.addEventListener('mouseleave', hide)
  node.addEventListener('blur', hide)
  node.addEventListener('mousemove', handleMove)
  window.addEventListener('scroll', hide, true)
  window.addEventListener('resize', hide)

  return {
    update(nextInput: TooltipInput) {
      const parsed = parseInput(nextInput)
      getText = parsed.getText
      delayMs = parsed.delayMs
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
      node.removeEventListener('mousemove', handleMove)
      window.removeEventListener('scroll', hide, true)
      window.removeEventListener('resize', hide)
    },
  }
}

