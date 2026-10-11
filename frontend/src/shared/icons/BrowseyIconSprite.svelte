<script lang="ts">
  import { iconSprite } from './iconSprite'
</script>

<svg class="icon-sprite" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" focusable="false" width="0" height="0">
  <defs>
    <!-- Static markup from bundled SVG assets, never user files or remote content. -->
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html iconSprite}
  </defs>
</svg>

<style>
  .icon-sprite { position: absolute; width: 0; height: 0; overflow: hidden; pointer-events: none; }
</style>
