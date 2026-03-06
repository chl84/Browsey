<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showLogLevelRow = false
  export let settings: { logLevel: 'error' | 'warn' | 'info' | 'debug' }
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeLogLevel: (value: 'error' | 'warn' | 'info' | 'debug') => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Advanced</div><div class="group-spacer"></div>

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
