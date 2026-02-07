<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import { autoSelectOnOpen } from '../../../ui/modalUtils'
  import type { Entry } from '../types'
  import type { SequenceMode } from '../modals/advancedRenameModal'

  export let open = false
  export let entries: Entry[] = []
  export let regex = ''
  export let replacement = ''
  export let caseSensitive = true
  export let sequenceMode: SequenceMode = 'numeric'
  export let sequenceStart = 1
  export let sequenceStep = 1
  export let sequencePad = 2
  export let error = ''
  export let onChange: (payload: {
    regex: string
    replacement: string
    caseSensitive: boolean
    sequenceMode: SequenceMode
    sequenceStart: number
    sequenceStep: number
    sequencePad: number
  }) => void = () => {}
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}

  let regexInput: HTMLInputElement | null = null
  let selectedThisOpen = false

  const handleChange = () => {
    onChange({
      regex,
      replacement,
      caseSensitive,
      sequenceMode,
      sequenceStart: Number(sequenceStart),
      sequenceStep: Number(sequenceStep),
      sequencePad: Number(sequencePad),
    })
  }

  $: autoSelectOnOpen({
    open,
    input: regexInput,
    selectedThisOpen,
    setSelected: (v: boolean) => (selectedThisOpen = v),
    value: regex,
  })

  const sequenceExample = sequenceMode === 'numeric' ? '001, 002, 003…' : 'AA, AB, AC…'
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onCancel}
    initialFocusSelector="input[name='regex']"
    guardOverlayPointer={true}
  >
    <svelte:fragment slot="header">Advanced Rename</svelte:fragment>

    {#if error}
      <div class="pill error">{error}</div>
    {/if}

    <div class="two-col">
      <label class="field">
        <span>Match (regex optional)</span>
        <input
          name="regex"
          type="text"
          autocomplete="off"
          bind:this={regexInput}
          bind:value={regex}
          on:input={handleChange}
        />
        <div class="muted">Leave empty to apply replacement/sequence to full name.</div>
        <label class="checkbox">
          <input type="checkbox" bind:checked={caseSensitive} on:change={handleChange} />
          <span>Case sensitive</span>
        </label>
      </label>

      <label class="field">
        <span>Replacement</span>
        <input type="text" autocomplete="off" bind:value={replacement} on:input={handleChange} />
        <div class="muted">Use $1, $2 for capture groups if regex is set.</div>
      </label>
    </div>

    <div class="two-col">
      <fieldset class="field sequence">
        <legend>Sequence</legend>
        <label class="radio">
          <input type="radio" name="seq-mode" value="numeric" bind:group={sequenceMode} on:change={handleChange} />
          <span>Numeric</span>
        </label>
        <label class="radio">
          <input type="radio" name="seq-mode" value="alpha" bind:group={sequenceMode} on:change={handleChange} />
          <span>Alphanumeric</span>
        </label>
        <div class="sequence-grid">
          <label>
            <span>Start</span>
            <input type="number" bind:value={sequenceStart} on:input={handleChange} />
          </label>
          <label>
            <span>Step</span>
            <input type="number" bind:value={sequenceStep} on:input={handleChange} />
          </label>
          <label>
            <span>Pad</span>
            <input type="number" min="0" max="8" bind:value={sequencePad} on:input={handleChange} />
          </label>
        </div>
        <div class="muted">Example: {sequenceExample}</div>
      </fieldset>

      <div class="field preview">
        <span>Preview</span>
        <div class="preview-box">
          {#if entries.length === 0}
            <div class="muted">No items selected</div>
          {:else}
            <ul>
              {#each entries.slice(0, 8) as entry}
                <li>
                  <span class="old">{entry.name}</span>
                  <span class="arrow">→</span>
                  <span class="new">(preview)</span>
                </li>
              {/each}
              {#if entries.length > 8}
                <li class="muted">+ {entries.length - 8} more…</li>
              {/if}
            </ul>
          {/if}
        </div>
      </div>
    </div>

    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={onConfirm}>Apply</button>
    </div>
  </ModalShell>
{/if}

<style>
  .two-col {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 16px;
    margin-bottom: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .checkbox,
  .radio {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 0.95rem;
    color: var(--fg);
  }

  .sequence {
    border: 1px solid var(--border);
    padding: 10px;
    gap: 8px;
  }

  .sequence-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(80px, 1fr));
    gap: 8px;
  }

  .preview-box {
    border: 1px solid var(--border);
    padding: 10px;
    border-radius: 2px;
    background: var(--bg-raised);
    min-height: 120px;
  }

  .preview ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .old {
    color: var(--fg-muted);
  }

  .new {
    color: var(--fg);
    font-weight: 600;
  }

  .arrow {
    margin: 0 6px;
    color: var(--fg-muted);
  }
</style>
