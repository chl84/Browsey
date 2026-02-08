<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import { autoSelectOnOpen } from '../../../ui/modalUtils'
  import type { Entry } from '../types'
  import type { AdvancedRenamePayload, SequenceMode } from '../modals/advancedRenameModal'
  import { computeAdvancedRenamePreview } from '../modals/advancedRenameUtils'

  export let open = false
  export let entries: Entry[] = []
  export let regex = ''
  export let replacement = ''
  export let prefix = ''
  export let suffix = ''
  export let caseSensitive = true
  export let sequenceMode: SequenceMode = 'none'
  export let sequenceStart = 1
  export let sequenceStep = 1
  export let sequencePad = 2
  export let error = ''
  export let onChange: (payload: AdvancedRenamePayload) => void = () => {}
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}

  let regexInput: HTMLInputElement | null = null
  let selectedThisOpen = false
  type PreviewRow = { original: string; next: string }
  let preview: PreviewRow[] = []
  let previewError = ''
  $: visibleError = error || previewError

  const handleChange = () => {
    onChange({
      regex,
      replacement,
      prefix,
      suffix,
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

  const sequenceExample =
    sequenceMode === 'numeric' ? '001, 002, 003…' : sequenceMode === 'alpha' ? 'AA, AB, AC…' : '—'

  $: {
    const { rows, error: nextError } = computeAdvancedRenamePreview(entries, {
      regex,
      replacement,
      prefix,
      suffix,
      caseSensitive,
      sequenceMode,
      sequenceStart,
      sequenceStep,
      sequencePad,
    })
    preview = rows
    previewError = nextError
  }
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onCancel}
    initialFocusSelector="input[name='regex']"
    guardOverlayPointer={true}
    modalClass="advanced-rename-modal"
    modalWidth="900px"
  >
    <svelte:fragment slot="header">Advanced Rename</svelte:fragment>

    <div class="content-scroll">
      {#if visibleError}
        <div class="pill error">{visibleError}</div>
      {/if}

      <div class="layout-grid">
        <div class="left-stack">
          <div class="panel">
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
              <div class="muted">Leave empty to apply replacement/prefix/suffix/sequence to full name.</div>
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

            <div class="field-row">
              <label class="field">
                <span>Prefix</span>
                <input type="text" autocomplete="off" bind:value={prefix} on:input={handleChange} />
              </label>

              <label class="field">
                <span>Suffix</span>
                <input type="text" autocomplete="off" bind:value={suffix} on:input={handleChange} />
              </label>
            </div>
          </div>

          <div class="panel">
            <fieldset class="field sequence">
              <legend>Sequence</legend>
              <label class="radio">
                <input type="radio" name="seq-mode" value="none" bind:group={sequenceMode} on:change={handleChange} />
                <span>None</span>
              </label>
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
                  <input type="number" bind:value={sequenceStart} on:input={handleChange} disabled={sequenceMode === 'none'} />
                </label>
                <label>
                  <span>Step</span>
                  <input type="number" bind:value={sequenceStep} on:input={handleChange} disabled={sequenceMode === 'none'} />
                </label>
                <label>
                  <span>Pad</span>
                  <input
                    type="number"
                    min="0"
                    max="8"
                    bind:value={sequencePad}
                    on:input={handleChange}
                    disabled={sequenceMode === 'none'}
                  />
                </label>
              </div>
              <div class="muted">Example: {sequenceExample}</div>
            </fieldset>
          </div>
        </div>

        <div class="panel preview preview-panel">
          <span>Preview</span>
          <div class="preview-box">
            {#if entries.length === 0}
              <div class="muted">No items selected</div>
            {:else}
              <ul>
                {#each entries as entry, idx}
                  <li>
                    <span class="old">{entry.name}</span>
                    <span class="arrow">→</span>
                    <span class="new">{preview[idx]?.next ?? '(preview)'}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
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
  :global(.modal.advanced-rename-modal) {
    overflow: hidden;
  }

  .content-scroll {
    display: flex;
    flex-direction: column;
    gap: var(--modal-gap);
    min-height: 0;
    flex: 1 1 auto;
    overflow: auto;
  }

  .layout-grid {
    display: flex;
    flex-wrap: wrap;
    gap: var(--modal-gap);
    margin-bottom: 0;
    min-height: 0;
  }

  .left-stack {
    display: flex;
    flex-direction: column;
    gap: var(--modal-gap);
    min-height: 0;
    flex: 1 1 240px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--modal-field-gap);
  }

  .field-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--modal-field-gap);
  }

  .checkbox,
  .radio {
    display: inline-flex;
    align-items: center;
    gap: var(--modal-field-gap);
    font-size: var(--modal-font-size);
    color: var(--fg);
  }

  .sequence {
    border: 1px solid var(--border);
    padding: var(--modal-input-padding-x);
    gap: var(--modal-field-gap);
  }

  .sequence-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: var(--modal-field-gap);
  }

  .sequence-grid input {
    width: 100%;
    box-sizing: border-box;
  }

  .preview-box {
    border: 1px solid var(--border);
    padding: var(--modal-input-padding-x);
    border-radius: 0;
    background: var(--bg-raised);
    width: 100%;
    box-sizing: border-box;
    height: 50vh;
    max-height: 50vh;
    overflow: auto;
  }

  .preview-panel {
    display: flex;
    flex-direction: column;
    gap: var(--modal-field-gap);
    width: 100%;
    margin-top: var(--modal-gap);
    flex: 1 1 360px;
  }

  @media (min-width: 700px) {
    .layout-grid {
      flex-wrap: nowrap;
      align-items: stretch;
    }
    .preview-panel {
      margin-top: 0;
      min-height: 0;
    }
    .preview-box {
      height: auto;
      max-height: none;
      min-height: 0;
      flex: 1 1 auto;
    }
  }

  @media (max-width: 699px) and (max-height: 760px) {
    .preview-box {
      height: 30vh;
      max-height: 30vh;
    }
  }

  @media (max-width: 640px) {
    .field-row {
      grid-template-columns: 1fr;
    }
  }

  .preview ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--modal-field-gap);
  }

  .old {
    color: var(--fg-muted);
  }

  .new {
    color: var(--fg);
    font-weight: 600;
  }

  .arrow {
    margin: 0 var(--modal-field-gap);
    color: var(--fg-muted);
  }
</style>
