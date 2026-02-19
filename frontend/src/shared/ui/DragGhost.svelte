<script lang="ts">
  export let visible = false
  export let x = 0
  export let y = 0
  export let count = 0
  export let allowed = true
  export let action: 'copy' | 'move' | null = null
</script>

{#if visible}
  {#if allowed}
    <div
      class="ghost"
      style={`left:${x + 12}px; top:${y + 12}px;`}
    >
      <div class="dot"></div>
      <div class="text">
        <div class="line">
          {#if action === 'copy'}
            Copy
          {:else}
            Move
          {/if}
          {count} item{count === 1 ? '' : 's'}
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .ghost {
    position: fixed;
    pointer-events: none;
    z-index: 9999;
    padding: 8px 12px;
    border-radius: 0;
    color: var(--drag-ghost-text);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 12px;
    box-shadow: var(--shadow-ghost);
    backdrop-filter: blur(4px);
    border: 1px solid var(--drag-ghost-border);
  }

  .ghost {
    background: var(--drag-ghost-bg);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 0;
    background: var(--drag-ghost-dot-bg);
  }

  .text {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }

  /* no sub-line when target is hidden */
</style>
