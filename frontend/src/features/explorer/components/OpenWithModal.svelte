<script lang="ts">
  import type { OpenWithApp, OpenWithChoice } from '../openWith'

  export let open = false
  export let path = ''
  export let apps: OpenWithApp[] = []
  export let loading = false
  export let error = ''
  export let busy = false
  export let onReload: () => void = () => {}
  export let onConfirm: (choice: OpenWithChoice) => void = () => {}
  export let onClose: () => void = () => {}

  let filter = ''
  let selected: string | null = null
  let customCommand = ''
  let customArgs = ''
  let overlayPointerDown = false
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
  <div
    class="overlay"
    role="presentation"
    tabindex="-1"
    on:pointerdown={(e) => {
      overlayPointerDown = e.target === e.currentTarget
    }}
    on:click={(e) => {
      if (overlayPointerDown && e.target === e.currentTarget) {
        onClose()
      }
      overlayPointerDown = false
    }}
    on:keydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
      }
    }}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      tabindex="0"
      on:click|stopPropagation
      on:keydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault()
          onClose()
        } else if (e.key === 'Enter' && hasSelection() && !busy) {
          e.preventDefault()
          confirm()
        }
      }}
    >
      <header>Open with…</header>
      <div class="path">
        <p class="muted">Path</p>
        <code>{path}</code>
      </div>

      <section class="block">
        <div class="row">
          <span class="label">Applications</span>
          <button type="button" class="ghost" on:click={onReload} disabled={loading || busy}>
            Refresh
          </button>
        </div>
        <input
          type="search"
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
                {#if app.comment}
                  <div class="app-comment">{app.comment}</div>
                {/if}
                <div class="app-exec">{app.exec}</div>
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
          placeholder="Executable or command"
          bind:value={customCommand}
          on:input={() => {
            if (customCommand.trim()) selected = null
          }}
        />
        <input
          type="text"
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

      <div class="actions">
        <button type="button" class="secondary" on:click={onClose} disabled={busy}>Cancel</button>
        <button type="button" on:click={confirm} disabled={!hasSelection() || busy}>
          {busy ? 'Opening…' : 'Open'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: grid;
    place-items: center;
    z-index: 30;
  }

  .modal {
    width: min(560px, 92vw);
    max-height: 92vh;
    overflow: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  header {
    font-weight: 700;
    font-size: 16px;
  }

  .path code {
    word-break: break-all;
    background: var(--bg-raised);
    padding: 8px;
    border-radius: 8px;
    border: 1px solid var(--border);
    display: block;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .label {
    font-weight: 700;
  }

  .muted {
    color: var(--fg-muted);
    margin: 0;
  }

  .small {
    font-size: 12px;
  }

  input[type='text'],
  input[type='search'],
  input:not([type]) {
    padding: 10px 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    border-radius: 8px;
  }

  input:focus {
    outline: 2px solid var(--border-accent);
  }

  .apps {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 260px;
    overflow: auto;
    padding-right: 2px;
  }

  .apps button {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 10px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    text-align: left;
  }

  .apps button:hover {
    border-color: var(--border-accent);
  }

  .apps button.selected {
    border-color: var(--border-accent);
    box-shadow: 0 0 0 1px var(--border-accent);
  }

  .app-head {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .app-name {
    font-weight: 700;
  }

  .app-comment {
    color: var(--fg-muted);
  }

  .app-exec {
    font-size: 12px;
    color: var(--fg-muted);
    word-break: break-all;
  }

  .pill {
    display: inline-flex;
    padding: 6px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-accent);
    background: var(--bg-alt);
    color: var(--fg);
    font-weight: 700;
    font-size: 12px;
  }

  .pill.small {
    padding: 4px 8px;
    font-size: 11px;
  }

  .pill.error {
    border-color: #c0392b;
    background: rgba(192, 57, 43, 0.15);
    color: #fceaea;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    color: var(--fg);
    cursor: pointer;
  }

  button.secondary {
    background: transparent;
  }

  button.ghost {
    background: transparent;
    border: 1px solid var(--border);
    padding: 6px 8px;
    font-size: 12px;
  }

  button:hover {
    border-color: var(--border-accent);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
