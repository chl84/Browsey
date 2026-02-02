<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import { autoSelectOnOpen } from '../../../ui/modalUtils'

  export let open = false
  export let value = ''
  export let level: number = 6
  export let error = ''
  export let onConfirm: (name: string, level: number) => void = () => {}
  export let onCancel: () => void = () => {}

  let inputEl: HTMLInputElement | null = null
  let selectedThisOpen = false
  const confirmAndClose = () => {
    onConfirm(value, Number(level))
    onCancel()
  }

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
    onClose={onCancel}
    initialFocusSelector="input[type='text']"
    guardOverlayPointer={true}
  >
    <svelte:fragment slot="header">Compress</svelte:fragment>

    {#if error}
      <div class="pill error">{error}</div>
    {/if}
    <label class="field">
      <span>Archive name</span>
      <div class="archive-input">
        <input
          type="text"
          id="compress-archive-name"
          autocomplete="off"
          bind:this={inputEl}
          bind:value={value}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              confirmAndClose()
            } else if (e.key === 'Escape') {
              e.preventDefault()
              onCancel()
            }
          }}
        />
        <span>.zip</span>
      </div>
    </label>
    <label class="field">
      <span>Compression level</span>
      <input
        type="range"
        id="compress-archive-level"
        autocomplete="off"
        min="0"
        max="9"
        step="1"
        bind:value={level}
      />
      <div class="muted">0 = store only, 9 = maximum compression</div>
    </label>
    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={confirmAndClose}>Create</button>
    </div>
  </ModalShell>
{/if}
