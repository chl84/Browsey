<script lang="ts">
  export let selectionText = ''
  export let hintText = ''
</script>

<footer class="statusbar">
  {#if selectionText}
    <span class="status-text">{selectionText}</span>
  {/if}
  {#if hintText}
    <span class="hint-pill">{hintText}</span>
  {/if}
</footer>

<style>
  .statusbar {
    height: 32px;
    border-top: none;
    background: var(--bg);
    border-radius: 0;
    margin-top: 0;
    position: sticky;
    bottom: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    gap: 12px;
    user-select: none;
  }

  .status-text {
    color: var(--fg);
    font-size: 14px;
    font-weight: 400;
  }

  .hint-pill {
    margin-left: auto;
    padding: 6px 10px;
    border-radius: 12px;
    background: var(--bg-raised);
    color: var(--fg);
    font-size: 13px;
    font-weight: 600;
    border: 1px solid var(--border);
    box-shadow: var(--shadow-sm);
    white-space: nowrap;
  }

</style>
