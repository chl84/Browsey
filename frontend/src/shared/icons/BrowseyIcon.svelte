<script lang="ts">
  import { iconSymbol } from './iconSprite'

  export let src = 'icons/scalable/browsey/shortcut.svg'
  export let className = ''
  export let label = ''
</script>

<svg
  class={`browsey-icon ${className}`.trim()}
  viewBox="0 0 128 128"
  aria-hidden={label ? undefined : 'true'}
  role={label ? 'img' : undefined}
  aria-label={label || undefined}
  focusable="false"
>
  <use href={`#${iconSymbol(src)}`} />
</svg>

<style>
  .browsey-icon { width: 100%; height: 100%; display: block; flex-shrink: 0; }
  :global(.row.selected:not(.drop-target)) .browsey-icon,
  :global(.card.selected:not(.drop-target)) .browsey-icon {
    --icon-paper: var(--icon-selected-paper);
    --icon-fold: var(--icon-selected-fold);
    --icon-ink: var(--icon-selected-ink);
    --icon-folder: var(--icon-selected-folder);
    --icon-folder-tab: var(--icon-selected-folder-tab);
    --icon-folder-detail: var(--icon-selected-folder-detail);
    --icon-folder-inset: var(--icon-selected-folder-inset);
    --icon-folder-edge: var(--icon-selected-folder-edge);
    --icon-blue: var(--icon-selected-blue);
    --icon-blue-mid: var(--icon-selected-blue-mid);
    --icon-blue-strong: var(--icon-selected-blue-strong);
    --icon-green: var(--icon-selected-green);
    --icon-green-mid: var(--icon-selected-green-mid);
    --icon-green-strong: var(--icon-selected-green-strong);
    --icon-purple: var(--icon-selected-purple);
    --icon-purple-mid: var(--icon-selected-purple-mid);
    --icon-orange: var(--icon-selected-orange);
    --icon-orange-mid: var(--icon-selected-orange-mid);
    --icon-orange-strong: var(--icon-selected-orange-strong);
    --icon-red: var(--icon-selected-red);
    --icon-on-red: var(--icon-selected-on-red);
    --icon-yellow: var(--icon-selected-yellow);
    --icon-on-yellow: var(--icon-selected-on-yellow);
    --icon-home: var(--icon-selected-home);
    --icon-home-shade: var(--icon-selected-home-shade);
    --icon-bookmark: var(--icon-selected-bookmark);
    --icon-bookmark-shade: var(--icon-selected-bookmark-shade);
    --icon-bookmark-edge: var(--icon-selected-bookmark-edge);
    --icon-neutral: var(--icon-selected-neutral);
    --icon-neutral-detail: var(--icon-selected-neutral-detail);
    --icon-neutral-link: var(--icon-selected-neutral-link);
    --icon-disk: var(--icon-selected-disk);
    --icon-disk-tab: var(--icon-selected-disk-tab);
    --icon-disk-detail: var(--icon-selected-disk-detail);
    --icon-disk-inset: var(--icon-selected-disk-inset);
    --icon-foreground: var(--icon-selected-foreground);
  }
</style>
