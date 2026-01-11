<script lang="ts">
  import BookmarksSection from './BookmarksSection.svelte'
  import PartitionsSection from './PartitionsSection.svelte'
  import PlacesSection from './PlacesSection.svelte'
  import type { Partition } from '../../explorer/types'

  export let places: { label: string; path: string }[] = []
  export let bookmarks: { label: string; path: string }[] = []
  export let partitions: Partition[] = []
  export let collapsed = false
  export let onPlaceSelect: (label: string, path: string) => void = () => {}
  export let onBookmarkSelect: (path: string) => void = () => {}
  export let onRemoveBookmark: (path: string) => void = () => {}
  export let onPartitionSelect: (path: string) => void = () => {}
</script>

<aside class="sidebar" class:collapsed={collapsed}>
  <PlacesSection places={places} onSelect={onPlaceSelect} />
  <BookmarksSection bookmarks={bookmarks} onSelect={onBookmarkSelect} onRemove={onRemoveBookmark} />
  <PartitionsSection partitions={partitions} onSelect={onPartitionSelect} />
</aside>

<style>
  .sidebar {
    background: var(--bg-alt);
    border: 1px solid var(--border-strong);
    border-radius: 0;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    overflow: auto;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .sidebar.collapsed {
    display: none;
  }
</style>
