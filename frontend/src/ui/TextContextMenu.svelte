<script lang="ts">
  import ContextMenu from '../features/explorer/components/ContextMenu.svelte'

  export let open = false
  export let x = 0
  export let y = 0
  export let target: HTMLElement | null = null
  export let readonly = false
  export let onClose: () => void = () => {}

  type ActionId = 'cut' | 'copy' | 'paste' | 'select-all'

  const actions: { id: ActionId; label: string; shortcut: string }[] = [
    { id: 'cut', label: 'Cut', shortcut: 'Ctrl+X' },
    { id: 'copy', label: 'Copy', shortcut: 'Ctrl+C' },
    { id: 'paste', label: 'Paste', shortcut: 'Ctrl+V' },
    { id: 'select-all', label: 'Select all', shortcut: 'Ctrl+A' },
  ]

  const isTextField = (el: HTMLElement | null): el is HTMLInputElement | HTMLTextAreaElement =>
    !!el &&
    (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) &&
    !['button', 'submit', 'reset', 'checkbox', 'radio', 'file'].includes(el.type)

  const focusTarget = (el: HTMLElement) => {
    if (typeof el.focus === 'function') el.focus()
  }

  const copySelection = async (el: HTMLElement) => {
    if (isTextField(el)) {
      const { selectionStart, selectionEnd, value } = el
      if (selectionStart === null || selectionEnd === null) return
      const text = value.slice(selectionStart, selectionEnd)
      if (text.length === 0) return
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text)
        return
      }
    }
    document.execCommand('copy')
  }

  const cutSelection = async (el: HTMLElement) => {
    if (isTextField(el)) {
      const { selectionStart, selectionEnd, value, readOnly, disabled } = el
      if (readOnly || disabled || selectionStart === null || selectionEnd === null) return
      const text = value.slice(selectionStart, selectionEnd)
      if (text.length === 0) return
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text)
      } else {
        document.execCommand('copy')
      }
      const next = value.slice(0, selectionStart) + value.slice(selectionEnd)
      el.value = next
      const pos = selectionStart
      el.selectionStart = pos
      el.selectionEnd = pos
      el.dispatchEvent(new Event('input', { bubbles: true }))
      return
    }
    document.execCommand('cut')
  }

  const pasteText = async (el: HTMLElement) => {
    let text = ''
    if (navigator.clipboard?.readText) {
      try {
        text = await navigator.clipboard.readText()
      } catch {
        // fall through to execCommand
      }
    }
    if (text === '') {
      const ok = document.execCommand('paste')
      if (!ok) return
      return
    }
    if (isTextField(el)) {
      const { selectionStart, selectionEnd, value, readOnly, disabled } = el
      if (readOnly || disabled || selectionStart === null || selectionEnd === null) return
      const next =
        value.slice(0, selectionStart) + text + value.slice(selectionEnd ?? selectionStart)
      el.value = next
      const pos = selectionStart + text.length
      el.selectionStart = pos
      el.selectionEnd = pos
      el.dispatchEvent(new Event('input', { bubbles: true }))
      return
    }
    document.execCommand('insertText', false, text)
  }

  const selectAll = (el: HTMLElement) => {
    if (isTextField(el)) {
      el.select()
      return
    }
    if (el.isContentEditable) {
      const range = document.createRange()
      range.selectNodeContents(el)
      const sel = window.getSelection()
      if (sel) {
        sel.removeAllRanges()
        sel.addRange(range)
      }
      return
    }
    document.execCommand('selectAll')
  }

  const handleSelect = async (id: ActionId) => {
    const el = target
    if (!el) return
    focusTarget(el)
    if (id === 'copy') await copySelection(el)
    else if (id === 'cut') await cutSelection(el)
    else if (id === 'paste') await pasteText(el)
    else if (id === 'select-all') selectAll(el)
    onClose()
  }

  $: filteredActions = actions.filter((a) => {
    if (!target) return false
    if (readonly && (a.id === 'cut' || a.id === 'paste')) return false
    return true
  })
</script>

<ContextMenu
  {open}
  {x}
  {y}
  actions={filteredActions}
  onSelect={(id) => void handleSelect(id as ActionId)}
  onClose={onClose}
/>
