import { tooltip, type TooltipContent } from '@/shared/ui/tooltip'

export function fullNameTooltip(node: HTMLElement, content: TooltipContent) {
  return tooltip(node, { content, delayMs: 750 })
}
