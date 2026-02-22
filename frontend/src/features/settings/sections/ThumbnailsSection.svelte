<script lang="ts">
  import Slider from '../../../shared/ui/Slider.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showVideoThumbsRow = false
  export let showFfmpegPathRow = false
  export let showThumbCacheRow = false
  export let thumbsDisabled = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleVideoThumbs: (value: boolean) => void = () => {}
  export let onChangeFfmpegPath: (value: string) => void = () => {}
  export let onChangeThumbCacheMb: (value: number) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Thumbnails</div><div class="group-spacer"></div>

  {#if showVideoThumbsRow}
    <div class="form-label">Video thumbs</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.videoThumbs}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ videoThumbs: next })
          onToggleVideoThumbs(next)
        }}
      />
      <span>Enable video thumbnails (requires ffmpeg)</span>
    </div>
  {/if}

  {#if showFfmpegPathRow}
    <div class="form-label">FFmpeg path</div>
    <div class="form-control">
      <input
        type="text"
        value={settings.ffmpegPath}
        placeholder="auto-detect if empty"
        disabled={thumbsDisabled}
        on:input={(e) => {
          const next = (e.currentTarget as HTMLInputElement).value
          onPatch({ ffmpegPath: next })
          onChangeFfmpegPath(next)
        }}
      />
    </div>
  {/if}

  {#if showThumbCacheRow}
    <div class="form-label">Cache size</div>
    <div class="form-control">
      <Slider
        min="50"
        max="1000"
        step="50"
        value={settings.thumbCacheMb}
        disabled={thumbsDisabled}
        on:input={(e) => {
          const next = e.detail.value
          onPatch({ thumbCacheMb: next })
          onChangeThumbCacheMb(next)
        }}
      />
      <small>{settings.thumbCacheMb} MB</small>
    </div>
  {/if}
{/if}
