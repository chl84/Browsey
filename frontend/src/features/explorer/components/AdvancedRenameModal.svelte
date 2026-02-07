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
  export let sequenceMode: SequenceMode = 'none'
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
  type PreviewRow = { original: string; next: string }
  let preview: PreviewRow[] = []
  let previewError = ''
  $: visibleError = error || previewError

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

  const sequenceExample =
    sequenceMode === 'numeric' ? '001, 002, 003…' : sequenceMode === 'alpha' ? 'AA, AB, AC…' : '—'

  const splitExt = (name: string) => {
    const dot = name.lastIndexOf('.')
    if (dot <= 0 || dot === name.length - 1) return { stem: name, ext: '' }
    return { stem: name.slice(0, dot), ext: name.slice(dot) }
  }

  const toAlpha = (n: number) => {
    if (n < 0) return ''
    let num = Math.floor(n)
    let out = ''
    do {
      out = String.fromCharCode(65 + (num % 26)) + out
      num = Math.floor(num / 26) - 1
    } while (num >= 0)
    return out
  }

  const formatSequence = (
    index: number,
    mode: SequenceMode,
    start: number,
    step: number,
    padWidth: number,
  ) => {
    const pad = Math.max(0, Number(padWidth) || 0)
    const value = Number(start) + index * Number(step || 0)
    if (mode === 'numeric') {
      const num = Number.isFinite(value) ? Math.round(value) : 0
      return Math.abs(pad) > 0 ? num.toString().padStart(pad, '0') : num.toString()
    }
    if (mode === 'alpha') {
      const alphaIndex = Math.max(0, Math.floor(value - 1))
      return toAlpha(alphaIndex)
    }
    return ''
  }

  const computePreview = (
    list: Entry[],
    patternSource: string,
    replace: string,
    isCaseSensitive: boolean,
    mode: SequenceMode,
    start: number,
    step: number,
    padWidth: number,
  ) => {
    let nextError = ''
    let pattern: RegExp | null = null
    const trimmed = patternSource.trim()
    if (trimmed.length > 0) {
      try {
        pattern = new RegExp(trimmed, isCaseSensitive ? 'g' : 'gi')
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err)
        nextError = `Invalid regex: ${msg}`
        pattern = null
      }
    }

    const rows = list.map((entry, idx) => {
      const seq = mode !== 'none' ? formatSequence(idx, mode, start, step, padWidth) : ''
      const source = entry.name

      let base = source
      if (pattern) {
        base = source.replace(pattern, replace)
      } else if (!trimmed && replace) {
        base = replace
      }

      let next = base
      if (mode !== 'none') {
        if (next.includes('$n')) {
          next = next.replaceAll('$n', seq)
        } else {
          const { stem, ext } = splitExt(next)
          next = `${stem}${seq}${ext}`
        }
      }

      return { original: source, next }
    })

    return { rows, error: nextError }
  }

  $: {
    const { rows, error: nextError } = computePreview(
      entries,
      regex,
      replacement,
      caseSensitive,
      sequenceMode,
      sequenceStart,
      sequenceStep,
      sequencePad,
    )
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
    modalWidth="900px"
  >
    <svelte:fragment slot="header">Advanced Rename</svelte:fragment>

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

    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={onConfirm}>Apply</button>
    </div>
  </ModalShell>
{/if}

<style>
  .layout-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 16px;
    margin-bottom: 12px;
    align-items: stretch;
    grid-auto-rows: 1fr;
  }

  @media (min-width: 900px) {
    .layout-grid {
      grid-template-columns: 1fr 2fr;
      grid-auto-rows: 1fr;
    }
    .preview-panel {
      grid-column: 2 / 3;
      grid-row: 1 / 2;
    }
  }

  .left-stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
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
    grid-template-columns: repeat(3, minmax(100px, 1fr));
    gap: 10px;
  }

  .sequence-grid input {
    width: 100%;
    box-sizing: border-box;
  }

  .preview-box {
    border: 1px solid var(--border);
    padding: 10px;
    border-radius: 2px;
    background: var(--bg-raised);
    min-height: 120px;
    height: 100%;
    overflow: auto;
    flex: 1;
  }

  .preview-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    margin-top: 0;
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
