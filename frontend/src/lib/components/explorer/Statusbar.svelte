<script lang="ts">
  export let selectionText = ''
</script>

<footer class="statusbar">
  {#if selectionText}
    <span class="status-text">{selectionText}</span>
  {/if}
</footer>

<style>
  .statusbar {
    height: 32px;
    border-top: 1px solid var(--border-strong);
    background: var(--bg-alt);
    border-radius: 0;
    margin-top: 0;
    position: sticky;
    bottom: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 8px;
  }

  .status-text {
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
