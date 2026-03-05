<script lang="ts">
  import Checkbox from '../../../shared/ui/Checkbox.svelte'
  import Slider from '../../../shared/ui/Slider.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showHardwareAccelerationRow = false
  export let showMountsPollRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleHardwareAcceleration: (value: boolean) => void = () => {}
  export let onChangeMountsPollMs: (value: number) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Performance</div><div class="group-spacer"></div>

  {#if showHardwareAccelerationRow}
    <div class="form-label">Hardware acceleration</div>
    <div class="form-control checkbox">
      <Checkbox
        checked={settings.hardwareAcceleration}
        on:change={(e) => {
          const next = (e.target as HTMLInputElement).checked
          onPatch({ hardwareAcceleration: next })
          onToggleHardwareAcceleration(next)
        }}
      >
        Use GPU acceleration for rendering
        <svelte:fragment slot="description">
          <small>Requires restart to take effect</small>
          <small>Leave this off unless rendering feels slow or unstable on your system.</small>
        </svelte:fragment>
      </Checkbox>
    </div>
  {/if}

  {#if showMountsPollRow}
    <div class="form-label">Mounts poll (ms)</div>
    <div class="form-control">
      <Slider
        min="500"
        max="10000"
        step="100"
        value={settings.mountsPollMs}
        on:input={(e) => {
          const next = e.detail.value
          onPatch({ mountsPollMs: next })
          onChangeMountsPollMs(next)
        }}
      />
      <small>{settings.mountsPollMs} ms</small>
    </div>
  {/if}
{/if}
