<script lang="ts">
  export let open = false
  export let conflicts: { src: string; target: string; is_dir: boolean }[] = []
  export let onOverwrite: () => void = () => {}
  export let onRenameAll: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <div
    class="modal-backdrop"
    role="presentation"
    tabindex="-1"
    on:click={onCancel}
    on:keydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onCancel()
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
          onCancel()
        }
      }}
    >
      <h2>Items already exist</h2>
      <p>{conflicts.length} item{conflicts.length === 1 ? '' : 's'} are already present in the destination.</p>
      <div class="conflicts">
        {#each conflicts as conflict (conflict.target)}
          <div class="row">
            <div class="name">
              {conflict.is_dir ? '📁' : '📄'} {conflict.src}
            </div>
            <div class="target">→ {conflict.target}</div>
          </div>
        {/each}
      </div>
      <div class="actions">
        <button class="secondary" type="button" on:click={onCancel}>Cancel</button>
        <button class="secondary" type="button" on:click={onRenameAll}>Auto-rename</button>
        <button class="primary" type="button" on:click={onOverwrite}>Overwrite</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9998;
  }
  .modal {
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 18px;
    min-width: 420px;
    max-height: 70vh;
    overflow: auto;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 18px;
  }
  p {
    margin: 0 0 10px;
    color: var(--fg-muted);
    font-size: 14px;
  }
  .conflicts {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 8px;
    max-height: 220px;
    overflow: auto;
    margin-bottom: 14px;
    background: var(--bg-alt);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--border);
  }
  .row:last-child {
    border-bottom: none;
  }
  .name {
    font-weight: 600;
  }
  .target {
    color: var(--fg-muted);
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  button {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    cursor: pointer;
    font-weight: 700;
  }
  button.primary {
    background: #1e88e5;
    border-color: #1e88e5;
    color: #fff;
  }
  button.secondary {
    background: var(--bg-alt);
  }
</style>
