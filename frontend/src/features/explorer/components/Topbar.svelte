<script lang="ts">
  export let pathInput = ''
  export let searchMode = false
  export let loading = false
  export let pathInputEl: HTMLInputElement | null = null
  export let onSubmitPath: () => void = () => {}
  export let onSearch: () => void = () => {}
  export let onExitSearch: () => void = () => {}
  export let onFocus: () => void = () => {}
  export let onBlur: () => void = () => {}
</script>

<header class="topbar">
  <div class="left">
    <div class="path">
      <input
        id="explorer-path-input"
        name="explorer-path"
        class="path-input"
        bind:value={pathInput}
        bind:this={pathInputEl}
        placeholder={searchMode ? 'Search in current folder…' : 'Path…'}
        aria-label={searchMode ? 'Search' : 'Path'}
        on:focus={onFocus}
        on:blur={onBlur}
        on:keydown={(e) => {
          if (e.key === 'Escape' && searchMode) {
            e.preventDefault()
            e.stopPropagation()
            onExitSearch()
            return
          }
          if (e.key === 'Enter' && !searchMode) {
            onSubmitPath()
          } else if (e.key === 'Enter' && searchMode) {
            onSearch()
          }
        }}
      />
    </div>
    {#if loading}
      <span class="pill">Loading…</span>
    {/if}
  </div>
</header>

<style>
  .topbar {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
    padding: 0;
  }

  .left {
    display: flex;
    gap: 12px;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .path {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    width: 100%;
    flex: 1;
    min-width: 0;
  }

  .path-input {
    flex: 1;
    min-width: 0;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0;
    padding: 10px 12px;
    background: var(--bg);
    color: var(--fg);
    font-size: 14px;
  }

  .path-input:focus {
    outline: 2px solid var(--border-accent);
    border-color: var(--border-accent-strong);
  }

  .pill {
    background: var(--bg-raised);
    color: var(--fg-pill);
    padding: 6px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    border: 1px solid var(--border);
  }
</style>
