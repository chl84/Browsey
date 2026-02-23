<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'
  import Slider from '../../../shared/ui/Slider.svelte'
  import { autoSelectOnOpen } from '../../../shared/ui/modalUtils'

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
            }
          }}
        />
        <span>.zip</span>
      </div>
    </label>
    <label class="field">
      <span>Compression level</span>
      <div class="level-row">
        <Slider
          id="compress-archive-level"
          autocomplete="off"
          min="0"
          max="9"
          step="1"
          bind:value={level}
        />
        <span class="level-value" aria-live="polite">{level}</span>
      </div>
      <div class="muted">0 = store only, 9 = maximum compression</div>
    </label>
    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={confirmAndClose}>Create</button>
    </div>
  </ModalShell>
{/if}

<style>
  .level-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--modal-field-gap);
  }

  .level-value {
    min-width: 1ch;
    text-align: right;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }
</style>
