<script lang="ts">
  export let message = ''
  export let tone: 'error' | 'info' = 'error'
</script>

{#if message}
  <div class:notice-error={tone === 'error'} class="notice">
    {tone === 'error' ? 'Error: ' : ''}{message}
  </div>
{/if}

<style>
  .notice {
    background: var(--bg-raised);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 12px 14px;
    border-radius: 0;
    font-weight: 600;
  }

  .notice-error {
    border-color: var(--border-accent);
    color: var(--accent-error-text);
  }
</style>
