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
    height: var(--statusbar-height);
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
    padding: 0 var(--statusbar-padding-x) 0 calc(var(--statusbar-padding-x) * 1.5);
    gap: var(--statusbar-gap);
    user-select: none;
  }

  .status-text {
    color: var(--fg);
    font-size: var(--statusbar-font-size);
    font-weight: 400;
  }

</style>
