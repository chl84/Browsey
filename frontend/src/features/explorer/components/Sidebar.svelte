<script lang="ts">
  import BookmarksSection from './BookmarksSection.svelte'
  import PartitionsSection from './PartitionsSection.svelte'
  import PlacesSection from './PlacesSection.svelte'
  import type { Partition } from '../types'

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
  <PlacesSection places={places} onSelect={onPlaceSelect} />
  <BookmarksSection bookmarks={bookmarks} onSelect={onBookmarkSelect} onRemove={onRemoveBookmark} />
  <PartitionsSection
    partitions={partitions}
    onSelect={onPartitionSelect}
    on:eject={(e) => onPartitionEject(e.detail.path)}
  />
</aside>

<style>
  .sidebar {
    background: var(--bg-alt);
    border-right: 1px solid var(--border-strong);
    border-top: none;
    border-bottom: none;
    border-left: none;
    border-radius: 0;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    overflow: auto;
    box-shadow: none;
    scrollbar-width: none;
    -ms-overflow-style: none;
    user-select: none;
    transform: translateX(0);
    transition: transform 500ms cubic-bezier(0.4, 0.0, 0.2, 1);
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .sidebar.collapsed {
    transform: translateX(-110%);
    pointer-events: none;
  }
</style>
