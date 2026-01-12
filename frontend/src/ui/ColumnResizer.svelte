<script lang="ts">
  export let onStart: (event: PointerEvent) => void = () => {}
</script>

<span class="resizer" on:pointerdown={onStart}></span>

<style>
  .resizer {
    flex: 0 0 10px;
    min-width: 10px;
    align-self: stretch;
    cursor: col-resize;
    display: inline-block;
    margin-left: 2px;
    border-radius: 4px;
    transition: background 120ms ease;
    position: relative;
    z-index: 2;
  }

  .resizer:hover {
    background: var(--border);
  }
</style>
