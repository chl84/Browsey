<script lang="ts">
  import SelectionBox from '../../../shared/ui/SelectionBox.svelte'
  import { iconPath as assetIconPath, formatSize, formatItems } from '../utils'
  import { iconPath as iconPathById } from '../icons'
  import type { Entry } from '../types'
  import { fullNameTooltip } from '../fullNameTooltip'
  import { createThumbnailLoader } from '../thumbnailLoader'
  import { onDestroy } from 'svelte'
  import { convertFileSrc } from '@/shared/lib/tauri'

  export let currentPath = ''
  export let videoThumbs = true
  export let thumbnailsEnabled = true
  export let thumbnailRefreshToken = 0

  const thumbLoader = createThumbnailLoader({
    maxConcurrent: 3,
    maxDim: 96,
    initialGeneration: currentPath,
    allowVideos: videoThumbs && thumbnailsEnabled,
  })
  let thumbMap = new Map<string, string>()
  const unsubThumbs = thumbLoader.subscribe((m) => {
    thumbMap = m
  })

  let lastPath = currentPath
  $: if (currentPath !== lastPath) {
    thumbLoader.reset(currentPath)
    lastPath = currentPath
  }

  let lastVideoThumbs = videoThumbs
  $: if (videoThumbs !== lastVideoThumbs) {
    thumbLoader.setAllowVideos(videoThumbs && thumbnailsEnabled)
    thumbLoader.reset(`${currentPath}-vthumbs-${videoThumbs ? 'on' : 'off'}`)
    lastVideoThumbs = videoThumbs
  }

  let lastThumbnailsEnabled = thumbnailsEnabled
  $: if (thumbnailsEnabled !== lastThumbnailsEnabled) {
    thumbLoader.setAllowVideos(videoThumbs && thumbnailsEnabled)
    thumbLoader.reset(`${currentPath}-thumbs-${thumbnailsEnabled ? 'on' : 'off'}`)
    lastThumbnailsEnabled = thumbnailsEnabled
  }

  let lastThumbnailRefreshToken = thumbnailRefreshToken
  $: if (thumbnailRefreshToken !== lastThumbnailRefreshToken) {
    thumbLoader.reset(`${currentPath}-thumb-refresh-${thumbnailRefreshToken}`)
    lastThumbnailRefreshToken = thumbnailRefreshToken
  }

  onDestroy(() => {
    unsubThumbs()
    thumbLoader.destroy()
  })

  export let entries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let start = 0
  export let offsetY = 0
  export let totalHeight = 0
  export let rowsEl: HTMLDivElement | null = null
  export let selected: Set<string> = new Set()
  export let clipboardMode: 'copy' | 'cut' = 'copy'
  export let clipboardPaths: Set<string> = new Set()
  export let isHidden: (entry: Entry) => boolean = () => false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let onWheel: (event: WheelEvent) => void = () => {}
  export let selectionActive = false
  export let selectionRect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
  export let onRowClick: (entry: Entry, index: number, event: MouseEvent) => void = () => {}
  export let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
  export let onRowDragStart: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnd: (event: DragEvent) => void = () => {}
  export let onRowDragEnter: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragOver: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDrop: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragLeave: (entry: Entry, event: DragEvent) => void = () => {}
  export let dragTargetPath: string | null = null
  export let dragAllowed = false
  export let dragging = false
  export let onRowsContextMenu: (event: MouseEvent) => void = () => {}
  export let onRowsClick: (event: MouseEvent) => void = () => {}
  export let onRowsMousedown: (event: MouseEvent) => void = () => {}
  export let onRowsScroll: (event: Event) => void = () => {}
  export let onRowsKeydown: (event: KeyboardEvent) => void = () => {}

  const readOnlyIcon = assetIconPath('status/eye-svgrepo-com.svg')
  const lockIcon = assetIconPath('status/padlock.svg')

  const observeThumb = (node: Element, payload: { entry: Entry; thumbnailsEnabled: boolean }) => {
    let binding: ReturnType<typeof thumbLoader.observe> | null = null

    const bind = (next: { entry: Entry; thumbnailsEnabled: boolean }) => {
      binding?.destroy()
      binding = null
      if (next.thumbnailsEnabled && next.entry.kind === 'file') {
        binding = thumbLoader.observe(node, next.entry.path)
      }
    }

    bind(payload)

    return {
      update(next: { entry: Entry; thumbnailsEnabled: boolean }) {
        bind(next)
      },
      destroy() {
        binding?.destroy()
      },
    }
  }
</script>

<section class="grid-container">
  <div
    class="grid"
    role="grid"
    tabindex="0"
    bind:this={rowsEl}
    style="user-select:none"
    on:scroll={onRowsScroll}
    on:wheel={onWheel}
    on:contextmenu={onRowsContextMenu}
    on:click={onRowsClick}
    on:mousedown={onRowsMousedown}
    on:keydown={onRowsKeydown}
  >
    {#if entries.length === 0}
      <div class="muted">No items here.</div>
    {:else}
      <div class="spacer" style={`height:${totalHeight}px`}>
        <div class="grid-viewport" style={`top:${offsetY}px;`}>
          {#each visibleEntries as entry, i (entry.path)}
            <button
              use:observeThumb={{ entry, thumbnailsEnabled }}
              class="card"
              class:dragging={dragging}
              class:selected={selected.has(entry.path)}
              class:cut={clipboardMode === 'cut' && clipboardPaths.has(entry.path)}
              class:hidden={isHidden(entry)}
              class:drop-target={dragTargetPath === entry.path}
              class:drop-blocked={dragTargetPath === entry.path && !dragAllowed}
              type="button"
              data-index={start + i}
              data-path={entry.path}
              draggable="true"
              on:click={(event) => onRowClick(entry, start + i, event)}
              on:contextmenu={(event) => {
                event.preventDefault()
                event.stopPropagation()
                onContextMenu(entry, event)
              }}
              on:dragstart={(event) => onRowDragStart(entry, event)}
              on:dragend={onRowDragEnd}
              on:dragenter|preventDefault={(event) => onRowDragEnter(entry, event)}
              on:dragover|preventDefault={(event) => onRowDragOver(entry, event)}
              on:dragleave={(event) => onRowDragLeave(entry, event)}
              on:drop|preventDefault={(event) => onRowDrop(entry, event)}
            >
              <div class="badges">
                {#if entry.readDenied}
                  <img class="badge-icon" src={lockIcon} alt="No read permission" title="No read permission" />
                {/if}
                {#if entry.readOnly}
                  <img class="badge-icon" src={readOnlyIcon} alt="Read-only" title="Read-only" />
                {/if}
              </div>
              {#if thumbnailsEnabled && thumbMap.has(entry.path)}
                <img
                  class="icon"
                  src={convertFileSrc(thumbMap.get(entry.path) || '')}
                  alt=""
                  draggable="false"
                  on:error={(e) => {
                    e.currentTarget?.setAttribute('src', iconPathById(entry.iconId))
                    thumbLoader.drop(entry.path)
                  }}
                />
              {:else}
                <img class="icon" src={iconPathById(entry.iconId)} alt="" draggable="false" />
              {/if}
              <div
                class="name"
                use:fullNameTooltip={() => {
                  const parts = [entry.name]
                  if (entry.kind === 'file' && entry.size !== null && entry.size !== undefined) {
                    parts.push(formatSize(entry.size))
                  } else if (entry.kind === 'dir' && entry.items !== null && entry.items !== undefined) {
                    const items = formatItems(entry.items)
                    if (items) parts.push(items)
                  }
                  return parts.join(' • ')
                }}
              >
                {displayName(entry)}
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    <SelectionBox active={selectionActive} rect={selectionRect} />
  </div>
</section>

<style>
  .grid-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 0;
  }

  .grid {
    display: block;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 0;
    position: relative;
    user-select: none;
    cursor: default;
    padding: 20px;
    box-sizing: border-box;
  }

  .grid:focus,
  .grid:focus-visible {
    outline: none;
  }

  .spacer {
    position: relative;
    width: 100%;
  }

  .grid-viewport {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, var(--grid-card-width));
    grid-auto-rows: var(--grid-row-height);
    gap: var(--grid-gap);
    justify-content: start;
    align-content: start;
    width: 100%;
  }

  .muted {
    color: var(--fg-muted);
    grid-column: 1 / -1;
    text-align: center;
    padding: 20px 0;
  }

  .card {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 8px 6px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 0;
    text-align: center;
    cursor: default;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    position: relative;
    overflow: hidden;
    transition: none;
  }

  .card:hover {
    background: var(--bg-hover);
    z-index: 1;
  }

  :global(.is-scrolling) .card:hover:not(.selected):not(.drop-target) {
    background: transparent;
    z-index: auto;
  }

  .card.selected {
    background: var(--selection-fill);
    border-color: transparent;
    box-shadow: none;
  }

  .card.cut {
    opacity: 0.55;
  }

  .card.drop-target {
    background: var(--drop-allowed-bg);
    border-color: var(--drop-allowed-border);
    box-shadow: var(--drop-allowed-shadow);
  }

  .card.drop-target.drop-blocked {
    background: var(--drop-blocked-bg);
    border-color: var(--drop-blocked-border);
    box-shadow: var(--drop-blocked-shadow);
  }

  .card.hidden {
    opacity: 0.55;
  }

  .card:focus-visible {
    outline: none;
  }

  .icon {
    width: var(--grid-thumb-size);
    height: var(--grid-thumb-size);
    object-fit: contain;
    display: block;
  }

  .badges {
    position: absolute;
    top: 4px;
    right: 4px;
    display: inline-flex;
    gap: 2px;
    flex-direction: column;
    align-items: flex-end;
  }

  .badge-icon {
    width: 16px;
    height: 16px;
    opacity: 0.65;
    flex-shrink: 0;
  }

  .name {
    font-weight: var(--font-weight-base);
    color: var(--fg-strong);
    line-height: 1.3;
    max-height: calc(1.3em * 3);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    box-orient: vertical;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    width: 100%;
    text-align: center;
  }

  .card.dragging * {
    pointer-events: none;
  }

  .card.dragging {
    pointer-events: auto;
  }

</style>
