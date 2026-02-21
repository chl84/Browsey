<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showExternalToolsRow = false
  export let showLogLevelRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Advanced</div><div class="group-spacer"></div>

  {#if showExternalToolsRow}
    <div class="form-label">External tools</div>
    <div class="form-control column">
      <textarea
        rows="2"
        value={settings.externalTools}
        placeholder="ffmpeg=/usr/bin/ffmpeg"
        on:input={(e) => onPatch({ externalTools: (e.currentTarget as HTMLTextAreaElement).value })}
      ></textarea>
    </div>
  {/if}

  {#if showLogLevelRow}
    <div class="form-label">Log level</div>
    <div class="form-control">
      <ComboBox
        value={settings.logLevel}
        on:change={(e) => onPatch({ logLevel: e.detail as Settings['logLevel'] })}
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
