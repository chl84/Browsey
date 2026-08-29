<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showDensityRow = false
  export let showThemeModeRow = false
  export let settings: Settings
  export let systemThemeName: string | null = null
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeDensity: (value: Settings['density']) => void = () => {}
  export let onChangeThemeMode: (value: Settings['themeMode']) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Appearance</div><div class="group-spacer"></div>
  {#if showThemeModeRow}
    <div class="form-label">Style</div>
    <div class="form-control">
      <ComboBox
        value={settings.themeMode}
        on:change={(e) => {
          const next = e.detail as Settings['themeMode']
          onPatch({ themeMode: next })
          onChangeThemeMode(next)
        }}
        options={[
          {
            value: 'system',
            label: systemThemeName ? `Use system style (Omarchy: ${systemThemeName})` : 'Use system style',
          },
          { value: 'dark', label: 'Dark' },
          { value: 'light', label: 'Light' },
        ] satisfies ComboOption[]}
      />
    </div>
  {/if}
  {#if showDensityRow}
    <div class="form-label">Density</div>
    <div class="form-control">
      <ComboBox
        value={settings.density}
        on:change={(e) => {
          const next = e.detail as Settings['density']
          onPatch({ density: next })
          onChangeDensity(next)
        }}
        options={[
          { value: 'cozy', label: 'Cozy' },
          { value: 'compact', label: 'Compact' },
        ] satisfies ComboOption[]}
      />
    </div>
  {/if}
{/if}
