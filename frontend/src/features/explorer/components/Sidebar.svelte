<script lang="ts">
  import BookmarksSection from './BookmarksSection.svelte'
  import PartitionsSection from './PartitionsSection.svelte'
  import PlacesSection from './PlacesSection.svelte'
  import type { Partition } from '../model/types'

  export let places: { label: string; path: string }[] = []
  export let bookmarks: { label: string; path: string }[] = []
  export let partitions: Partition[] = []
  export let collapsed = false
  export let onPlaceSelect: (label: string, path: string) => void = () => {}
  export let onBookmarkSelect: (path: string) => void = () => {}
  export let onRemoveBookmark: (path: string) => void = () => {}
  export let onPartitionSelect: (path: string) => void = () => {}
  export let onPartitionEject: (path: string) => void = () => {}
</script>

<aside class="sidebar" class:collapsed={collapsed}>
  <div class="drag-top" data-tauri-drag-region></div>
  <div class="sidebar-scroll">
    <PlacesSection places={places} onSelect={onPlaceSelect} />
    <BookmarksSection bookmarks={bookmarks} onSelect={onBookmarkSelect} onRemove={onRemoveBookmark} />
    <PartitionsSection
      partitions={partitions}
      onSelect={onPartitionSelect}
      on:eject={(e) => onPartitionEject(e.detail.path)}
    />
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
    width: auto;
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
</style>
