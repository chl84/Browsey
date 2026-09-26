<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'
  import Slider from '../../../shared/ui/Slider.svelte'
  import { autoSelectOnOpen } from '../../../shared/ui/modalUtils'

  export let open = false
  export let value = ''
  export let level: number = 6
  export let error = ''
  export let onConfirm: (name: string, level: number, password?: string) => void | Promise<void> = () => {}
  export let onCancel: () => void = () => {}

  let inputEl: HTMLInputElement | null = null
  let selectedThisOpen = false
  let busy = false
  let protect = false
  let password = ''
  let confirmation = ''
  let showPassword = false
  let passwordError = ''
  $: if (!open || !protect) {
    password = ''; confirmation = ''; showPassword = false; passwordError = ''
    if (!open) protect = false
  }
  const confirm = async () => {
    if (busy) return
    passwordError = ''
    if (protect && (!password || password.includes('\0') || password !== confirmation)) {
      passwordError = !password ? 'Enter a password.' : password.includes('\0') ? 'Password must not contain NUL characters.' : 'Passwords do not match.'
      return
    }
    busy = true
    try {
      const pending = onConfirm(value, Number(level), protect ? password : undefined)
      password = ''; confirmation = ''; showPassword = false
      await pending
    } finally {
      busy = false
    }
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
    onClose={() => { if (!busy) onCancel() }}
    closeOnEscape={!busy}
    closeOnOverlay={!busy}
    initialFocusSelector="input[type='text']"
    guardOverlayPointer={true}
  >
    <svelte:fragment slot="header">Compress</svelte:fragment>

    {#if error}
      <div class="pill error" role="alert">{error}</div>
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
          disabled={busy}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              void confirm()
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
          disabled={busy}
        />
        <span class="level-value" aria-live="polite">{level}</span>
      </div>
      <div class="muted">0 = store only, 9 = maximum compression</div>
    </label>
    <label><input type="checkbox" bind:checked={protect} disabled={busy} /> Protect with password</label>
    {#if protect}
      <p class="muted">AES-256 encryption. File names remain visible. Some older ZIP tools cannot open encrypted ZIP files.</p>
      {#if passwordError}<div class="pill error" role="alert">{passwordError}</div>{/if}
      <label class="field">
        <span>Password</span>
        <input type={showPassword ? 'text' : 'password'} bind:value={password} disabled={busy}
          autocomplete="new-password" spellcheck={false} autocapitalize="none" />
      </label>
      <label class="field">
        <span>Confirm password</span>
        <input type={showPassword ? 'text' : 'password'} bind:value={confirmation} disabled={busy}
          autocomplete="new-password" spellcheck={false} autocapitalize="none"
          on:keydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); void confirm() } }} />
      </label>
      <label><input type="checkbox" bind:checked={showPassword} disabled={busy} /> Show password</label>
    {/if}
    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>{busy ? 'Cancel compression' : 'Cancel'}</button>
      <button type="button" on:click={confirm} disabled={busy}>{busy ? 'Compressing…' : 'Create'}</button>
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
