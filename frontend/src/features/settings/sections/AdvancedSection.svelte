<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import TextField from '../../../shared/ui/TextField.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showRclonePathRow = false
  export let showLogLevelRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeRclonePath: (value: string) => void = () => {}
  export let onChangeLogLevel: (value: Settings['logLevel']) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Advanced</div><div class="group-spacer"></div>

  {#if showRclonePathRow}
    <div class="form-label">Rclone path</div>
    <div class="form-control column">
      <TextField
        type="text"
        value={settings.rclonePath}
        placeholder="auto-detect if empty"
        on:input={(e) => {
          const next = (e.currentTarget as HTMLInputElement).value
          onPatch({ rclonePath: next })
          onChangeRclonePath(next)
        }}
      />
    </div>
  {/if}

  {#if showLogLevelRow}
    <div class="form-label">Log level</div>
    <div class="form-control">
      <ComboBox
        value={settings.logLevel}
        on:change={(e) => {
          const next = e.detail as Settings['logLevel']
          onPatch({ logLevel: next })
          onChangeLogLevel(next)
        }}
        options={[
          { value: 'error', label: 'Error' },
          { value: 'warn', label: 'Warn' },
          { value: 'info', label: 'Info' },
          { value: 'debug', label: 'Debug' },
        ] satisfies ComboOption[]}
      />
    </div>
  {/if}
{/if}
