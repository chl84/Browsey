<script lang="ts">
  import { slide } from 'svelte/transition'
  import { onDestroy } from 'svelte'

  export let message = ''
  export let tone: 'error' | 'info' = 'error'
  export let duration = 3000

  let visible = false
  let hideTimer: ReturnType<typeof setTimeout> | null = null

  const clearTimer = () => {
    if (hideTimer) {
      clearTimeout(hideTimer)
      hideTimer = null
    }
  }

  $: {
    if (message) {
      visible = true
      clearTimer()
      hideTimer = setTimeout(() => {
        visible = false
      }, duration)
    } else {
      visible = false
      clearTimer()
    }
  }

  onDestroy(clearTimer)
</script>

{#if visible && message}
  <div
    class:notice-error={tone === 'error'}
    class="notice"
    in:slide={{ duration: 360 }}
    out:slide={{ duration: 360 }}
  >
    {tone === 'error' ? 'Error: ' : ''}{message}
  </div>
{/if}

<style>
  .notice {
    background: var(--bg-raised);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 8px 10px;
    border-radius: 0;
    font-weight: 500;
    font-size: 13px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .notice-error {
    border-color: var(--border-accent);
    color: var(--accent-error-text);
  }
</style>
