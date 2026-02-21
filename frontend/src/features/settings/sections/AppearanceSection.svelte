<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showDensityRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeDensity: (value: Settings['density']) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Appearance</div><div class="group-spacer"></div>
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
