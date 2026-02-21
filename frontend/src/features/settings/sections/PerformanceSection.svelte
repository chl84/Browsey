<script lang="ts">
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
      <input
        type="checkbox"
        checked={settings.hardwareAcceleration}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ hardwareAcceleration: next })
          onToggleHardwareAcceleration(next)
        }}
      />
      <span>Use GPU acceleration for rendering</span>
      <small>Requires restart to take effect</small>
    </div>
  {/if}

  {#if showMountsPollRow}
    <div class="form-label">Mounts poll (ms)</div>
    <div class="form-control">
      <input
        type="range"
        min="500"
        max="10000"
        step="100"
        value={settings.mountsPollMs}
        on:input={(e) => {
          const next = Number((e.currentTarget as HTMLInputElement).value)
          onPatch({ mountsPollMs: next })
          onChangeMountsPollMs(next)
        }}
      />
      <small>{settings.mountsPollMs} ms</small>
    </div>
  {/if}
{/if}
