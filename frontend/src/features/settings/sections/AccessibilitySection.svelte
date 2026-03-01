<script lang="ts">
  import Slider from '../../../shared/ui/Slider.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showHighContrastRow = false
  export let showScrollbarWidthRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleHighContrast: (value: boolean) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Accessibility</div><div class="group-spacer"></div>

  {#if showHighContrastRow}
    <div class="form-label">High contrast</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.highContrast}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ highContrast: next })
          onToggleHighContrast(next)
        }}
      />
      <span>Boost contrast for UI elements</span>
    </div>
  {/if}

  {#if showScrollbarWidthRow}
    <div class="form-label">Scrollbar width</div>
    <div class="form-control">
      <Slider
        min="6"
        max="16"
        step="1"
        value={settings.scrollbarWidth}
        on:input={(event) => onPatch({ scrollbarWidth: event.detail.value })}
      />
      <small>{settings.scrollbarWidth} px</small>
    </div>
  {/if}
{/if}
