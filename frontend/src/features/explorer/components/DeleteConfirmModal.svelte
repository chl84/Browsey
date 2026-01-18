<script lang="ts">
  export let open = false
  export let targetLabel = ''
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <div
    class="overlay danger-overlay"
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
      <header>Delete permanently?</header>
      <p class="muted">This cannot be undone.</p>
      <p class="path">{targetLabel}</p>
      <div class="actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button
          type="button"
          class="danger"
          on:click={onConfirm}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              onConfirm()
            }
          }}
        >
          Delete
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
