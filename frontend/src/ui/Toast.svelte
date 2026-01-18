<script lang="ts">
  export let message: string | null = null
</script>

{#if message}
  <div class="toast">{message}</div>
{/if}

<style>
  .toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: #1c2027;
    color: var(--fg);
    padding: 10px 14px;
    border-radius: 0;
    border: 1px solid var(--border);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
    z-index: 30;
    max-width: 300px;
    font-size: 13px;
  }
</style>
