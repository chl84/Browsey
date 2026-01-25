<script lang="ts">
  export let pathInput = ''
  export let searchMode = false
  export let mode: 'address' | 'filter' | 'search' = 'address'
  export let loading = false
  export let activity:
    | { label: string; percent: number | null; cancel?: (() => void) | null; cancelling?: boolean }
    | null = null
  export let pathInputEl: HTMLInputElement | null = null
  export let onSubmitPath: () => void = () => {}
  export let onSearch: () => void = () => {}
  export let onExitSearch: () => void = () => {}
  export let onFocus: () => void = () => {}
  export let onBlur: () => void = () => {}
  export let onNavigateSegment: (path: string) => void = () => {}

  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'

  const appWindow = getCurrentWindow()

  let theme: 'light' | 'dark' = 'dark'

  onMount(() => {
    const stored = localStorage.getItem('browsey-theme')
    const current = stored === 'light' ? 'light' : 'dark'
    applyTheme(current)
  })

  const applyTheme = (next: 'light' | 'dark') => {
    theme = next
    const root = document.documentElement
    if (next === 'light') {
      root.dataset.theme = 'light'
    } else {
      root.removeAttribute('data-theme')
    }
    localStorage.setItem('browsey-theme', next)
  }

  const toggleTheme = () => {
    applyTheme(theme === 'dark' ? 'light' : 'dark')
  }

  const minimize = () => {
    void appWindow.minimize()
  }

  const toggleMaximize = async () => {
    try {
      await appWindow.toggleMaximize()
    } catch (err) {
      console.error('toggleMaximize failed', err)
    }
  }

  const closeWindow = () => {
    void appWindow.close()
  }

  let focused = false
  let suppressMouseUp = false

  const isWindows = typeof navigator !== "undefined" && navigator.userAgent.toLowerCase().includes("windows")

  const detectSeparator = (path: string) => (path.includes('\\') && !path.includes('/') ? '\\' : '/')

  const buildBreadcrumbs = (path: string) => {
    if (!path) return []
    const sep = detectSeparator(path)
    const driveMatch = path.match(/^[A-Za-z]:/)
    const crumbs: { label: string; path: string }[] = []
    let remainder = path

    if (driveMatch) {
      const drive = driveMatch[0]
      const drivePath = `${drive}${sep}`
      crumbs.push({ label: drive, path: drivePath })
      remainder = path.slice(drive.length)
      remainder = sep === '\\' ? remainder.replace(/^\\+/, '') : remainder.replace(/^\/+/, '')
    } else if (path.startsWith(sep)) {
      crumbs.push({ label: sep, path: sep })
      remainder = path.slice(1)
    }

    const parts = remainder.split(/[\\/]+/).filter((p) => p.length > 0)
    let acc = crumbs.length > 0 ? crumbs[crumbs.length - 1].path : ''
    for (const part of parts) {
      acc = acc ? `${acc}${acc.endsWith(sep) ? '' : sep}${part}` : part
      crumbs.push({ label: part, path: acc })
    }

    if (crumbs.length === 0) {
      crumbs.push({ label: path, path })
    }

    return crumbs
  }

  $: showBreadcrumbs = !focused && !searchMode && mode === 'address'
  $: separatorChar = detectSeparator(pathInput)
  $: breadcrumbs = buildBreadcrumbs(pathInput)
</script>

<div class="drag-spacer" data-tauri-drag-region>
  <div class="window-controls" aria-label="Window controls">
    <button
      class="theme-toggle"
      class:light={theme === 'light'}
      type="button"
      aria-label="Toggle theme"
      aria-pressed={theme === 'light'}
      on:click|stopPropagation={toggleTheme}
    >
      <span class="icon sun" aria-hidden="true">☀</span>
      <span class="icon moon" aria-hidden="true">☾</span>
      <span class="thumb" aria-hidden="true"></span>
      <span class="sr-only">{theme === 'light' ? 'Light mode' : 'Dark mode'}</span>
    </button>
    <button class="win-btn minimize" type="button" aria-label="Minimize window" on:click|stopPropagation={minimize}>–</button>
    <button class="win-btn maximize" type="button" aria-label="Toggle maximize window" on:click|stopPropagation={toggleMaximize}>⬜</button>
    <button class="win-btn close" type="button" aria-label="Close window" on:click|stopPropagation={closeWindow}>×</button>
  </div>
</div>

<header class="topbar">
  <div class="left">
    <div class="path">
      <input
        id="explorer-path-input"
        name="explorer-path"
        class="path-input"
        class:hidden={showBreadcrumbs}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        bind:value={pathInput}
        bind:this={pathInputEl}
        placeholder={searchMode ? 'Search in current folder…' : 'Path…'}
        aria-label={searchMode ? 'Search' : 'Path'}
        on:focus={() => {
          focused = true
          suppressMouseUp = true
          pathInputEl?.select()
          onFocus()
        }}
        on:blur={() => {
          focused = false
          onBlur()
        }}
        on:mouseup={(e) => {
          if (suppressMouseUp) {
            e.preventDefault()
            suppressMouseUp = false
          }
        }}
        on:keydown={(e) => {
          if (e.key === 'Backspace' && mode === 'filter' && pathInput.length <= 1) {
            e.preventDefault()
            e.stopPropagation()
            onExitSearch()
            return
          }
          if (e.key === 'Escape') {
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

      {#if showBreadcrumbs}
        <div class="breadcrumb-bar" aria-label="Path breadcrumbs">
          {#each breadcrumbs as crumb, i}
            <button class="crumb" type="button" on:click={() => onNavigateSegment(crumb.path)}>
              {crumb.label || separatorChar}
            </button>
            {#if i < breadcrumbs.length - 1}
              <span class="sep" class:windows={isWindows}>{separatorChar}</span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
    {#if loading}
      <span class="pill">Loading…</span>
    {/if}
    {#if activity}
      <div class="pill progress" class:cancelling={activity.cancelling}>
        <span>{activity.label}</span>
        {#if activity.percent !== null}
          <div class="progress-bar" aria-hidden="true">
            <div class="progress-fill" style={`width:${Math.min(100, Math.max(0, activity.percent))}%;`}></div>
          </div>
          <span class="percent">{Math.min(100, Math.max(0, activity.percent)).toFixed(0)}%</span>
        {/if}
        {#if activity.cancel}
          <button
            class="pill-cancel"
            type="button"
            aria-label="Cancel task"
            on:click={(e) => {
              e.stopPropagation()
              activity?.cancel?.()
            }}
            disabled={activity.cancelling}
          >
            &times;
          </button>
        {/if}
      </div>
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

  .drag-spacer {
    height: 32px;
    width: 100%;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 0 0 10px; /* no right padding so buttons align with scrollbar edge */
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
    position: relative;
  }

  .path-input {
    flex: 1;
    min-width: 0;
    width: 100%;
    border: none;
    border-radius: 0;
    padding: 10px 12px;
    background: var(--bg);
    color: var(--fg);
    font-size: 14px;
  }

  .path-input:focus {
    outline: none;
    border-color: transparent;
  }

  .path-input.hidden {
    opacity: 0;
  }

  .pill.progress {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    padding: 6px 10px;
    border-radius: 0;
    font-size: 12px;
  }

  .pill.progress.cancelling {
    opacity: 0.85;
  }

  .pill-cancel {
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 1px 4px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    line-height: 1;
  }

  .pill-cancel:hover:not(:disabled) {
    color: var(--fg);
  }

  .pill-cancel:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .progress-bar {
    width: 80px;
    height: 6px;
    border-radius: 0;
    background: var(--border);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--border-accent);
    border-radius: 0;
    transition: width 120ms ease;
  }

  .percent {
    font-variant-numeric: tabular-nums;
    color: var(--fg-muted);
  }

  .breadcrumb-bar {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 10px 12px;
    color: var(--fg);
    pointer-events: none;
    overflow: hidden;
    white-space: nowrap;
  }

  .crumb {
    pointer-events: auto;
    border: none;
    background: transparent;
    color: var(--fg);
    padding: 6px 10px;
    border-radius: 0;
    font-size: 13px;
    line-height: 1.2;
    display: inline-flex;
    align-items: center;
    transition: background 120ms ease;
  }

  .crumb:focus,
  .crumb:focus-visible {
    outline: none;
    box-shadow: none;
  }

  .crumb:hover {
    background: var(--bg-hover);
  }

  .sep {
    pointer-events: none;
    color: var(--fg-muted);
    font-size: 13px;
    display: inline-flex;
    align-items: center;
    line-height: 1;
    transform: translateY(0);
  }

  .sep.windows {
    transform: translateY(-3px);
  }

  .pill {
    background: var(--bg-raised);
    color: var(--fg-pill);
    padding: 6px 10px;
    border-radius: 0;
    font-size: 12px;
    font-weight: 600;
    border: 1px solid var(--border);
  }

  .window-controls {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .theme-toggle {
    position: relative;
    width: 46px;
    height: 20px;
    border: 1px solid var(--win-btn-border);
    background: linear-gradient(120deg, var(--bg-raised), var(--bg));
    border-radius: 0;
    padding: 2px 2px 2px 2px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background 140ms ease, border-color 140ms ease;
    color: var(--fg-muted);
    overflow: hidden;
    margin-right: 10px;
  }

  .theme-toggle .icon {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    font-size: 12px;
    line-height: 1;
    opacity: 0.45;
    z-index: 2;
    user-select: none;
    pointer-events: none;
  }

  .theme-toggle .icon.sun {
    left: 4px;
  }

  .theme-toggle .icon.moon {
    right: 6px;
  }

  .theme-toggle.light .icon.sun {
    opacity: 1;
    color: #535353;
  }

  .theme-toggle.light .icon.moon {
    opacity: 0.35;
  }

  .theme-toggle:not(.light) .icon.moon {
    opacity: 1;
    color: #cbd5e1;
  }

  .theme-toggle:not(.light) .icon.sun {
    opacity: 0.35;
  }

  .theme-toggle .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 15px;
    border-radius: 0;
    background: #f5ce0b;
    transform: translateX(0);
    transition: transform 140ms ease, background 160ms ease, color 160ms ease, box-shadow 160ms ease;
  }

  .theme-toggle:not(.light) .thumb {
    transform: translateX(24px);
    background: linear-gradient(135deg, #111827, #1f2937);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .win-btn {
    width: 20px;
    height: 20px;
    box-sizing: border-box;
    border: 1px solid var(--win-btn-border);
    background: var(--win-btn-bg);
    color: var(--win-btn-fg);
    font-size: 12px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: default;
    padding: 0;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .win-btn:hover {
    background: var(--win-btn-hover-bg);
  }

  .win-btn.close:hover {
    background: var(--win-btn-close-hover-bg);
    color: var(--win-btn-close-hover-fg);
    border-color: var(--win-btn-close-hover-border);
  }
</style>
