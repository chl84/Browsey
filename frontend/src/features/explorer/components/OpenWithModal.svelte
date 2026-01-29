<script lang="ts">
  import type { OpenWithApp, OpenWithChoice } from '../services/openWith'

  import ModalShell from '../../../ui/ModalShell.svelte'

  export let open = false
  export let apps: OpenWithApp[] = []
  export let loading = false
  export let error = ''
  export let busy = false
  export let onConfirm: (choice: OpenWithChoice) => void = () => {}
  export let onClose: () => void = () => {}

  let filter = ''
  let selected: string | null = null
  let customCommand = ''
  let customArgs = ''
  let filtered: OpenWithApp[] = []

  $: {
    if (!open) {
      filter = ''
      selected = null
      customCommand = ''
      customArgs = ''
    } else if (selected && apps.every((a) => a.id !== selected)) {
      selected = null
    } else if (!selected && apps.length > 0 && !customCommand.trim()) {
      selected = apps[0].id
    }
  }

  $: filtered = filter
    ? apps.filter((app) => {
        const needle = filter.toLowerCase()
        const comment = app.comment ? app.comment.toLowerCase() : ''
        return (
          app.name.toLowerCase().includes(needle) ||
          comment.includes(needle) ||
          app.exec.toLowerCase().includes(needle)
        )
      })
    : apps

  const confirm = () => {
    if (busy) return
    onConfirm({
      appId: selected,
      customCommand: customCommand.trim() || undefined,
      customArgs: customArgs.trim() || undefined,
    })
  }
  const hasSelection = () => Boolean(selected) || Boolean(customCommand.trim())
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onClose}
    modalClass="open-with-modal"
    initialFocusSelector="input[type='search']"
    guardOverlayPointer={true}
  >
    <svelte:fragment slot="header">Open with…</svelte:fragment>

    <section class="block">
      <input
        type="search"
        id="open-with-filter"
        autocomplete="off"
        placeholder="Filter apps"
        bind:value={filter}
        on:keydown={(e) => {
          if (e.key === 'Enter' && hasSelection() && !busy) {
            e.preventDefault()
            confirm()
          }
        }}
      />
      <div class="apps">
        {#if loading}
          <div class="muted">Loading apps…</div>
        {:else if filtered.length === 0}
          <div class="muted">No associated applications found. Add a custom command below.</div>
        {:else}
          {#each filtered as app}
            <button
              type="button"
              class:selected={selected === app.id}
              on:click={() => {
                selected = app.id
                customCommand = ''
                customArgs = ''
              }}
              disabled={busy}
            >
              <div class="app-head">
                <span class="app-name">{app.name}</span>
                {#if app.matches}
                  <span class="pill small">Recommended</span>
                {/if}
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </section>

    <section class="block">
      <div class="label">Custom command</div>
      <div class="muted small">Use this if your app is missing from the list. File path is appended.</div>
      <input
        type="text"
        id="open-with-custom-command"
        autocomplete="off"
        placeholder="Executable or command"
        bind:value={customCommand}
        on:input={() => {
          if (customCommand.trim()) selected = null
        }}
      />
      <input
        type="text"
        id="open-with-custom-args"
        autocomplete="off"
        placeholder="Optional arguments"
        bind:value={customArgs}
        on:keydown={(e) => {
          if (e.key === 'Enter' && hasSelection() && !busy) {
            e.preventDefault()
            confirm()
          }
        }}
      />
    </section>

    {#if error}
      <div class="pill error">{error}</div>
    {/if}

    <div slot="actions">
      <button type="button" class="secondary" on:click={onClose} disabled={busy}>Cancel</button>
      <button type="button" on:click={confirm} disabled={!hasSelection() || busy}>
        {busy ? 'Opening…' : 'Open'}
      </button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
