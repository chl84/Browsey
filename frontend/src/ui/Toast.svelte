<script lang="ts">
  export let message: string | null = null
</script>

{#if message}
  <div class="toast">
    <span class="text">{message}</span>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: var(--bg-raised);
    color: var(--fg);
    padding: 10px 14px;
    border-radius: 0;
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    z-index: 30;
    max-width: 300px;
    font-size: 13px;
    display: inline-block;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .text {
    display: inline;
  }
</style>
