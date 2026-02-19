<script lang="ts">
  export let active = false
  export let rect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
</script>

{#if active && (rect.width > 0 || rect.height > 0)}
  <div
    class="selection-box"
    style={`left:${rect.x}px; top:${rect.y}px; width:${rect.width}px; height:${rect.height}px;`}
  ></div>
{/if}

<style>
  .selection-box {
    position: absolute;
    border: 1px solid var(--selection-border);
    background: var(--selection-fill);
    pointer-events: none;
    border-radius: 0;
    box-shadow: none;
  }
</style>
