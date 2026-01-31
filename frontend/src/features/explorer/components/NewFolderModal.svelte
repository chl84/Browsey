<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import { autoSelectOnOpen } from '../../../ui/modalUtils'

  export let open = false
  export let value = ''
  export let error = ''
  export let title = 'New folder'
  export let confirmLabel = 'Create'
  export let inputId = 'new-folder-name'
  export let onConfirm: (name: string) => void = () => {}
  export let onCancel: () => void = () => {}

  let inputEl: HTMLInputElement | null = null
  let selectedThisOpen = false

  $: autoSelectOnOpen({
    open,
    input: inputEl,
    selectedThisOpen,
    setSelected: (v: boolean) => (selectedThisOpen = v),
    value,
  })
</script>

{#if open}
  <ModalShell
    open={open}
    modalWidth="360px"
    onClose={onCancel}
    initialFocusSelector="input"
  >
    <svelte:fragment slot="header">{title}</svelte:fragment>

    {#if error}
      <div class="pill error">{error}</div>
    {/if}

    <input
      id={inputId}
      autocomplete="off"
      bind:this={inputEl}
      bind:value={value}
      on:keydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          onConfirm(value)
        } else if (e.key === 'Escape') {
          e.preventDefault()
          onCancel()
        }
      }}
    />

    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={() => onConfirm(value)}>{confirmLabel}</button>
    </div>
  </ModalShell>
{/if}
