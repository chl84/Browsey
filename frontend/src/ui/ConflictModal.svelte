<script lang="ts">
  export let open = false
  export let conflicts: { src: string; target: string; is_dir: boolean }[] = []
  export let onOverwrite: () => void = () => {}
  export let onRenameAll: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <div
    class="overlay conflict-overlay"
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
      class="modal conflict-modal"
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
  /* Styling is inherited from global modal rules in app.css */
</style>
