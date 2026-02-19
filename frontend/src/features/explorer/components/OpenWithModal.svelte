<script lang="ts">
  import type { OpenWithApp, OpenWithChoice } from '../services/openWith.service'

  import ModalShell from '../../../shared/ui/ModalShell.svelte'

  export let open = false
  export let apps: OpenWithApp[] = []
  export let loading = false
  export let error = ''
  export let busy = false
  export let onConfirm: (choice: OpenWithChoice) => void = () => {}
  export let onClose: () => void = () => {}

  let filter = ''
  let selected: string | null = null
  let filtered: OpenWithApp[] = []

  $: {
    if (!open) {
      filter = ''
      selected = null
    } else if (selected && apps.every((a) => a.id !== selected)) {
      selected = null
    } else if (!selected && apps.length > 0) {
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
    })
  }
  const hasSelection = () => Boolean(selected)
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
  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .apps {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 260px;
    overflow: auto;
    padding: 4px 4px 4px 0; /* add right padding for scrollbar */
  }

  .apps button {
    width: 100%;
    text-align: left;
    padding: 12px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .apps button.selected {
    border-color: var(--border-accent);
    background: var(--bg-raised);
  }

  .apps button:hover:not(:disabled) {
    border-color: var(--border-accent-strong);
  }

  .app-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
  }

  .app-name {
    font-weight: 600;
    line-height: 1.4;
  }

  /* align the pill to center baseline */
  .app-head .pill.small {
    display: inline-flex;
    align-items: center;
    line-height: 1;
    padding: 4px 8px;
  }

  input[type='search'] {
    width: 100%;
  }
</style>
