<script lang="ts">
  import Slider from '../../../shared/ui/Slider.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showDoubleClickRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeDoubleClickMs: (value: number) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Interaction</div><div class="group-spacer"></div>

  {#if showDoubleClickRow}
    <div class="form-label">Double-click speed</div>
    <div class="form-control">
      <Slider
        min="150"
        max="600"
        step="10"
        value={settings.doubleClickMs}
        on:input={(e) => {
          const next = e.detail.value
          onPatch({ doubleClickMs: next })
          onChangeDoubleClickMs(next)
        }}
      />
      <small>{settings.doubleClickMs} ms</small>
    </div>
  {/if}
{/if}
