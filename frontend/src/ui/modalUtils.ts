import { tick } from 'svelte'

export type AutoSelectStrategy = 'all' | 'basename'

/**
 * Focus and select text when a modal opens.
 * Resets the `selectedThisOpen` flag when the modal is closed.
 */
export async function autoSelectOnOpen(params: {
  open: boolean
  input: HTMLInputElement | null
  selectedThisOpen: boolean
  setSelected: (value: boolean) => void
  strategy?: AutoSelectStrategy
  value?: string
}) {
  const { open, input, selectedThisOpen, setSelected, strategy = 'all', value = '' } = params

  if (!open) {
    if (selectedThisOpen) setSelected(false)
    return
  }
  if (!input || selectedThisOpen) return

  await tick()
  if (!open || !input || selectedThisOpen) return

  if (strategy === 'basename') {
    selectBaseName(input, value)
  } else {
    selectAll(input)
  }
  setSelected(true)
}

export function selectAll(input: HTMLInputElement | null) {
  if (!input) return
  input.focus()
  input.select()
}

export function selectBaseName(input: HTMLInputElement | null, value: string) {
  if (!input) return
  input.focus()
  const dot = value.lastIndexOf('.')
  if (dot > 0) {
    input.setSelectionRange(0, dot)
  } else {
    input.select()
  }
}
