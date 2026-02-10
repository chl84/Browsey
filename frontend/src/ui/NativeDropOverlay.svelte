<script lang="ts">
  export let visible = false
  export let x: number | null = null
  export let y: number | null = null
</script>

{#if visible}
  <div class="overlay" aria-hidden="true">
    <div
      class="bubble"
      style={`transform: translate(${x ?? 0}px, ${y ?? 0}px);`}
    >
      Drop files to open
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 90;
    pointer-events: none;
    background: var(--overlay-soft);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .bubble {
    padding: 14px 18px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow-xl);
    font-weight: 700;
    color: var(--fg);
    transition: transform 80ms ease-out;
    min-width: 180px;
    text-align: center;
  }
</style>
