<script lang="ts">
  export let visible = false
  export let x = 0
  export let y = 0
  export let count = 0
  export let allowed = true
  export let target: string | null = null
</script>

{#if visible}
  {#if allowed}
    <div
      class="ghost"
      style={`left:${x + 12}px; top:${y + 12}px;`}
    >
      <div class="dot"></div>
      <div class="text">
        <div class="line">{count} item{count === 1 ? '' : 's'}</div>
        {#if target}
          <div class="sub">to {target}</div>
        {/if}
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
    color: #fff;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 12px;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.25);
    backdrop-filter: blur(4px);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .ghost {
    background: linear-gradient(120deg, rgba(25, 118, 210, 0.95), rgba(55, 158, 255, 0.9));
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 0;
    background: rgba(255, 255, 255, 0.9);
  }

  .text {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }

  .sub {
    opacity: 0.8;
    font-weight: 600;
  }
</style>
