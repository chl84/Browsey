<script lang="ts">
  export let selectionText = ''
  export let hintText = ''
</script>

<footer class="statusbar">
  {#if selectionText}
    <span class="status-text">{selectionText}</span>
  {/if}
  {#if hintText}
    <span class="pill status-pill">{hintText}</span>
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

  .status-pill {
    margin-left: auto;
    white-space: nowrap;
  }

</style>
