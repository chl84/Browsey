<script lang="ts">
import BookmarksSection from '../../components/BookmarksSection.svelte'
import PartitionsSection from '../../components/PartitionsSection.svelte'
import PlacesSection from '../../components/PlacesSection.svelte'
import TextField from '../../../../shared/ui/TextField.svelte'
import { filterSidebarEntries } from '../sidebarFilter'
import type { Partition } from '../../model/types'

  export let places: { label: string; path: string }[] = []
  export let bookmarks: { label: string; path: string }[] = []
  export let partitions: Partition[] = []
  export let collapsed = false
  export let onPlaceSelect: (label: string, path: string) => void = () => {}
  export let onBookmarkSelect: (path: string) => void = () => {}
  export let onRemoveBookmark: (path: string) => void = () => {}
  export let dragTargetPath: string | null = null
  export let onBookmarkDragOver: (path: string, e: DragEvent) => void = () => {}
  export let onBookmarkDragLeave: (path: string, e: DragEvent) => void = () => {}
  export let onBookmarkDrop: (path: string, e: DragEvent) => void = () => {}
  export let onPartitionSelect: (path: string) => void = () => {}
  export let onPartitionEject: (path: string) => void = () => {}

  let sidebarFilter = ''
  let filteredPlaces = places
  let filteredBookmarks = bookmarks
  let filteredPartitions = partitions
  let hasFilter = false
  let hasMatches = true

  $: ({ places: filteredPlaces, bookmarks: filteredBookmarks, partitions: filteredPartitions } =
    filterSidebarEntries({
      query: sidebarFilter,
      places,
      bookmarks,
      partitions,
    }))
  $: hasFilter = sidebarFilter.trim().length > 0
  $: hasMatches =
    filteredPlaces.length > 0 || filteredBookmarks.length > 0 || filteredPartitions.length > 0
</script>

<aside class="sidebar" class:collapsed={collapsed}>
  <div class="drag-top" data-tauri-drag-region></div>
  <div class="sidebar-scroll">
    <div class="sidebar-filter-wrap">
      <TextField
        type="search"
        variant="sidebar"
        className="sidebar-filter"
        placeholder="Filter sidebar"
        aria-label="Filter sidebar"
        bind:value={sidebarFilter}
      />
    </div>

    {#if filteredPlaces.length > 0}
      <PlacesSection places={filteredPlaces} onSelect={onPlaceSelect} />
    {/if}

    {#if filteredBookmarks.length > 0}
      <BookmarksSection
        bookmarks={filteredBookmarks}
        {dragTargetPath}
        onSelect={onBookmarkSelect}
        onRemove={onRemoveBookmark}
        onDragOver={onBookmarkDragOver}
        onDragLeave={onBookmarkDragLeave}
        onDrop={onBookmarkDrop}
      />
    {/if}

    {#if filteredPartitions.length > 0}
      <PartitionsSection
        partitions={filteredPartitions}
        onSelect={onPartitionSelect}
        on:eject={(e) => onPartitionEject(e.detail.path)}
      />
    {/if}

    {#if hasFilter && !hasMatches}
      <div class="sidebar-empty">No sidebar matches</div>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    background: var(--bg-alt);
    border-right: 1px solid var(--border-strong);
    border-top: none;
    border-bottom: none;
    border-left: none;
    border-radius: 0;
    padding: var(--sidebar-padding);
    display: flex;
    flex-direction: column;
    gap: 0;
    height: 100%;
    overflow: hidden;
    box-shadow: none;
    scrollbar-width: none;
    -ms-overflow-style: none;
    user-select: none;
    width: var(--sidebar-max-width);
    max-width: var(--sidebar-max-width);
    transition:
      max-width 600ms cubic-bezier(0.4, 0.0, 0.2, 1),
      padding 600ms cubic-bezier(0.4, 0.0, 0.2, 1),
      border-right-width 600ms cubic-bezier(0.4, 0.0, 0.2, 1);
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .sidebar.collapsed {
    max-width: 0;
    padding-left: 0;
    padding-right: 0;
    border-right-width: 0;
    pointer-events: none;
  }

  .drag-top {
    position: sticky;
    top: 0;
    height: var(--sidebar-drag-height);
    background: var(--bg-alt);
    z-index: 1;
  }

  .sidebar-scroll {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--sidebar-gap);
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  .sidebar-scroll::-webkit-scrollbar {
    display: none;
  }

  .sidebar-filter-wrap {
    padding: 2px 6px 0;
  }

  .sidebar-empty {
    color: var(--fg-dim);
    font-size: var(--font-size-small);
    padding: 0 10px;
  }
</style>
