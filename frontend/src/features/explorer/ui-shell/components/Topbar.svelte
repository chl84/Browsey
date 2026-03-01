<script lang="ts">
  export let pathInput = ''
  export let searchMode = false
  export let mode: 'address' | 'filter' = 'address'
  export let loading = false
  export let viewMode: 'list' | 'grid' = 'list'
  export let showHidden = false
  export let activity:
    | {
        label: string
        detail?: string | null
        percent: number | null
        cancel?: (() => void) | null
        cancelling?: boolean
      }
    | null = null
  export let pathInputEl: HTMLInputElement | null = null
  export let onSubmitPath: () => void = () => {}
  export let onSearch: () => void = () => {}
  export let onExitSearch: () => void = () => {}
  export let onFocus: () => void = () => {}
  export let onBlur: () => void = () => {}
  export let onGoBack: () => void = () => {}
  export let onGoForward: () => void = () => {}
  export let onNavigateSegment: (path: string) => void = () => {}
  export let onBreadcrumbDragOver: (path: string, e: DragEvent) => void = () => {}
  export let onBreadcrumbDragLeave: (path: string, e: DragEvent) => void = () => {}
  export let onBreadcrumbDrop: (path: string, e: DragEvent) => void = () => {}
  export let dragTargetPath: string | null = null
  export let onMainMenuAction: (
    id: 'open-settings' | 'open-shortcuts' | 'search' | 'toggle-hidden' | 'refresh' | 'about'
  ) => void = () => {}
  export let onToggleViewMode: (mode: 'list' | 'grid') => void = () => {}

import { getCurrentWindow } from '@tauri-apps/api/window'
import ThemeToggle from '../../components/ThemeToggle.svelte'
import TopbarActionMenu from './TopbarActionMenu.svelte'
import PulseTravelIndicator from '@/shared/ui/PulseTravelIndicator.svelte'

  const appWindow = getCurrentWindow()

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

  let menuButtonEl: HTMLButtonElement | null = null
  let actionMenuOpen = false
  let actionMenuX = 0
  let actionMenuY = 0

  const toggleActionMenu = () => {
    if (actionMenuOpen) {
      actionMenuOpen = false
      return
    }
    if (!menuButtonEl || typeof window === 'undefined') return
    const rect = menuButtonEl.getBoundingClientRect()
    actionMenuX = rect.left
    actionMenuY = rect.bottom + 4
    actionMenuOpen = true
  }

  const closeActionMenu = () => {
    actionMenuOpen = false
  }

  let focused = false
  let suppressMouseUp = false

  const isWindows = typeof navigator !== "undefined" && navigator.userAgent.toLowerCase().includes("windows")

  const detectSeparator = (path: string) => (path.includes('\\') && !path.includes('/') ? '\\' : '/')

  const buildRcloneBreadcrumbs = (path: string) => {
    if (!path.startsWith('rclone://')) return null
    const rest = path.slice('rclone://'.length)
    const parts = rest.split('/').filter((p) => p.length > 0)
    if (parts.length === 0) {
      return [{ label: path, path }]
    }

    const [remote, ...segments] = parts
    const crumbs: { label: string; path: string }[] = []
    let acc = `rclone://${remote}`
    crumbs.push({ label: remote, path: acc })
    for (const segment of segments) {
      acc = `${acc}/${segment}`
      crumbs.push({ label: segment, path: acc })
    }
    return crumbs
  }

  const buildBreadcrumbs = (path: string) => {
    if (!path) return []
    const rcloneCrumbs = buildRcloneBreadcrumbs(path)
    if (rcloneCrumbs) return rcloneCrumbs
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
      const rootLabel = 'Root'
      crumbs.push({ label: rootLabel, path: sep })
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
    <ThemeToggle />
    <button
      bind:this={menuButtonEl}
      class="win-btn menu-btn"
      class:active={actionMenuOpen}
      type="button"
      aria-label="Main menu"
      aria-haspopup="menu"
      aria-expanded={actionMenuOpen}
      on:click|stopPropagation={toggleActionMenu}
    >
      <svg class="menu-icon" viewBox="0 0 10 7" aria-hidden="true" focusable="false">
        <line x1="0" y1="0.5" x2="10" y2="0.5"></line>
        <line x1="0" y1="3.5" x2="10" y2="3.5"></line>
        <line x1="0" y1="6.5" x2="10" y2="6.5"></line>
      </svg>
    </button>
    <button class="win-btn minimize" type="button" aria-label="Minimize window" on:click|stopPropagation={minimize}>–</button>
    <button class="win-btn maximize" type="button" aria-label="Toggle maximize window" on:click|stopPropagation={toggleMaximize}>□</button>
    <button class="win-btn close" type="button" aria-label="Close window" on:click|stopPropagation={closeWindow}>×</button>
  </div>
</div>

<TopbarActionMenu
  open={actionMenuOpen}
  x={actionMenuX}
  y={actionMenuY}
  gridMode={viewMode === 'grid'}
  {showHidden}
  onClose={closeActionMenu}
  onSelect={(id) => {
    onMainMenuAction(id)
    closeActionMenu()
  }}
  onToggleViewMode={(nextGridMode) => {
    onToggleViewMode(nextGridMode ? 'grid' : 'list')
    closeActionMenu()
  }}
/>

<header class="topbar">
  <div class="left">
    <div class="nav-buttons">
      <button class="nav-btn" type="button" aria-label="Go back" on:click={onGoBack}>
        <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <path d="M10.5 3.5L6 8l4.5 4.5" />
        </svg>
      </button>
      <button class="nav-btn" type="button" aria-label="Go forward" on:click={onGoForward}>
        <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <path d="M5.5 3.5L10 8l-4.5 4.5" />
        </svg>
      </button>
    </div>
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
          if (e.key === 'Enter') {
            if (searchMode) {
              onSearch()
            } else if (mode === 'address') {
              onSubmitPath()
            }
          }
        }}
      />

      {#if showBreadcrumbs}
        <div class="breadcrumb-bar" aria-label="Path breadcrumbs">
          {#each breadcrumbs as crumb, i}
            <button
              class="crumb"
              class:drop-target={dragTargetPath === crumb.path}
              type="button"
              on:click={() => onNavigateSegment(crumb.path)}
              on:dragenter|preventDefault={(e) => onBreadcrumbDragOver(crumb.path, e)}
              on:dragover|preventDefault={(e) => onBreadcrumbDragOver(crumb.path, e)}
              on:dragleave={(e) => onBreadcrumbDragLeave(crumb.path, e)}
              on:drop|preventDefault={(e) => onBreadcrumbDrop(crumb.path, e)}
            >
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
        {#if activity.detail}
          <span class="detail">{activity.detail}</span>
        {/if}
        {#if activity.percent !== null}
          <div class="progress-bar" aria-hidden="true">
            <div class="progress-fill" style={`width:${Math.min(100, Math.max(0, activity.percent))}%;`}></div>
          </div>
          <span class="percent">{Math.min(100, Math.max(0, activity.percent)).toFixed(0)}%</span>
        {:else}
          <PulseTravelIndicator />
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
    gap: var(--topbar-gap);
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
    height: var(--topbar-drag-height);
    width: 100%;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 0 0 var(--topbar-input-padding-x); /* no right padding so buttons align with scrollbar edge */
  }

  .left {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-shrink: 0;
    padding-left: calc(var(--topbar-input-padding-x) + 15px);
  }

  .nav-btn {
    width: var(--topbar-control-size);
    height: var(--topbar-control-size);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg-muted);
    border-radius: 0;
    padding: 0;
    cursor: default;
  }

  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--fg);
    border-color: var(--border-strong);
  }

  .nav-btn:focus,
  .nav-btn:focus-visible {
    outline: none;
  }

  .nav-btn svg {
    width: 14px;
    height: 14px;
    stroke: currentColor;
    stroke-width: 1.5;
    fill: none;
    stroke-linecap: square;
    stroke-linejoin: miter;
  }

  .path {
    display: flex;
    gap: calc(var(--topbar-gap) - 4px);
    align-items: center;
    flex-wrap: wrap;
    width: 100%;
    flex: 1;
    min-width: 0;
    position: relative;
    margin-left: 3px;
  }

  .path-input {
    flex: 1;
    min-width: 0;
    width: 100%;
    border: none;
    border-radius: 0;
    padding: var(--topbar-input-padding-y) var(--topbar-input-padding-x);
    background: var(--bg);
    color: var(--fg);
    font-size: var(--topbar-input-font-size);
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
    gap: calc(var(--topbar-gap) - 4px);
    background: var(--bg-alt);
    border: 1px solid var(--border);
    padding: var(--topbar-pill-padding-y) var(--topbar-pill-padding-x);
    border-radius: 0;
    font-size: var(--topbar-pill-font-size);
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
    font-size: var(--topbar-pill-cancel-font-size);
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
    width: var(--topbar-progress-width);
    height: var(--topbar-progress-height);
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

  .detail {
    font-variant-numeric: tabular-nums;
    color: var(--fg-muted);
  }

  .breadcrumb-bar {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: var(--topbar-breadcrumb-gap);
    padding: var(--topbar-input-padding-y) var(--topbar-input-padding-x);
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
    padding: var(--topbar-crumb-padding-y) var(--topbar-crumb-padding-x);
    border-radius: 0;
    font-size: var(--topbar-crumb-font-size);
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

  .crumb.drop-target {
    background: var(--drop-allowed-bg);
    border: 1px solid var(--drop-allowed-border);
    padding: var(--topbar-crumb-drop-padding-y) var(--topbar-crumb-drop-padding-x);
  }

  .sep {
    pointer-events: none;
    color: var(--fg-muted);
    font-size: var(--topbar-separator-font-size);
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
    padding: var(--topbar-pill-padding-y) var(--topbar-pill-padding-x);
    border-radius: 0;
    font-size: var(--topbar-pill-font-size);
    font-weight: 600;
    border: 1px solid var(--border);
  }

  .window-controls {
    display: inline-flex;
    align-items: center;
    gap: var(--topbar-window-controls-gap);
    flex-shrink: 0;
  }

  .win-btn {
    width: var(--topbar-win-btn-size);
    height: var(--topbar-win-btn-size);
    box-sizing: border-box;
    border: 1px solid var(--win-btn-border);
    background: var(--win-btn-bg);
    color: var(--win-btn-fg);
    font-size: var(--topbar-win-btn-font-size);
    font-family: 'Segoe UI Symbol', 'Segoe UI', system-ui, sans-serif;
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

  .menu-btn {
    padding: 0;
  }

  .menu-btn.active {
    background: var(--win-btn-hover-bg);
    border-color: var(--border-accent);
  }

  .menu-icon {
    width: 10px;
    height: 7px;
    display: block;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    stroke-linecap: butt;
    vector-effect: non-scaling-stroke;
    transform: translateY(0.5px);
    flex: 0 0 auto;
  }

  .win-btn.close:hover {
    background: var(--win-btn-close-hover-bg);
    color: var(--win-btn-close-hover-fg);
    border-color: var(--win-btn-close-hover-border);
  }
</style>
