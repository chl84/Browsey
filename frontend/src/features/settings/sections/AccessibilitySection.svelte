<script lang="ts">
  import Checkbox from '../../../shared/ui/Checkbox.svelte'
  import Slider from '../../../shared/ui/Slider.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showHighContrastRow = false
  export let showScrollbarWidthRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleHighContrast: (value: boolean) => void = () => {}
  export let onChangeScrollbarWidth: (value: number) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Accessibility</div><div class="group-spacer"></div>

  {#if showHighContrastRow}
    <div class="form-label">High contrast</div>
    <div class="form-control checkbox">
      <Checkbox
        checked={settings.highContrast}
        on:change={(e) => {
          const next = (e.target as HTMLInputElement).checked
          onPatch({ highContrast: next })
          onToggleHighContrast(next)
        }}
      >
        Boost contrast for UI elements
      </Checkbox>
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
        on:input={(event) => {
          const next = event.detail.value
          onPatch({ scrollbarWidth: next })
          onChangeScrollbarWidth(next)
        }}
      />
      <small>{settings.scrollbarWidth} px</small>
    </div>
  {/if}
{/if}
