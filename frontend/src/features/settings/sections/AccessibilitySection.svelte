<script lang="ts">
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showHighContrastRow = false
  export let showScrollbarWidthRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  const onNumberInput = (key: 'scrollbarWidth') => (event: Event) => {
    const target = event.currentTarget as HTMLInputElement
    onPatch({ [key]: Number(target.value) } as Partial<Settings>)
  }
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
        on:change={(e) => onPatch({ highContrast: (e.currentTarget as HTMLInputElement).checked })}
      />
      <span>Boost contrast for UI elements</span>
    </div>
  {/if}

  {#if showScrollbarWidthRow}
    <div class="form-label">Scrollbar width</div>
    <div class="form-control">
      <input
        type="range"
        min="6"
        max="16"
        step="1"
        value={settings.scrollbarWidth}
        on:input={onNumberInput('scrollbarWidth')}
      />
      <small>{settings.scrollbarWidth} px</small>
    </div>
  {/if}
{/if}
