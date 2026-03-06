<script lang="ts">
  import type { CloudSetupStatus } from '@/features/network'
  import { describeCloudSetupStatus } from '../cloudSetup'
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import TextField from '../../../shared/ui/TextField.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showRclonePathRow = false
  export let showLogLevelRow = false
  export let settings: Settings
  export let cloudSetupStatus: CloudSetupStatus | null = null
  export let cloudSetupStatusBusy = false
  export let cloudSetupStatusError = ''
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeRclonePath: (value: string) => Promise<void> | void = () => {}
  export let onChangeLogLevel: (value: Settings['logLevel']) => void = () => {}
  $: setupCopy = describeCloudSetupStatus(cloudSetupStatus)
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Advanced</div><div class="group-spacer"></div>

  {#if showRclonePathRow}
    <div class="form-label">Cloud setup</div>
    <div class="form-control">
      <div class="cloud-setup-card" aria-live="polite">
        <div class="cloud-setup-headline">{setupCopy.headline}</div>
        <div class="cloud-setup-next-step">{setupCopy.nextStep}</div>

        {#if cloudSetupStatus?.resolvedBinaryPath}
          <div class="cloud-setup-meta">
            <span class="cloud-setup-meta-label">Binary</span>
            <code>{cloudSetupStatus.resolvedBinaryPath}</code>
          </div>
        {/if}

        {#if cloudSetupStatus}
          <div class="cloud-setup-meta">
            <span class="cloud-setup-meta-label">Remotes</span>
            <span>
              {cloudSetupStatus.supportedRemoteCount} supported
              {#if cloudSetupStatus.unsupportedRemoteCount > 0}
                , {cloudSetupStatus.unsupportedRemoteCount} unsupported
              {/if}
            </span>
          </div>
        {/if}

        {#if cloudSetupStatus?.state === 'ready' && cloudSetupStatus.supportedRemotes.length > 0}
          <div class="cloud-setup-remotes">
            {#each cloudSetupStatus.supportedRemotes as remote}
              <span class="cloud-setup-remote">{remote.label}</span>
            {/each}
          </div>
        {/if}

        {#if cloudSetupStatusBusy}
          <div class="cloud-setup-busy">Refreshing status…</div>
        {/if}

        {#if cloudSetupStatusError}
          <div class="cloud-setup-error">{cloudSetupStatusError}</div>
        {/if}
      </div>
    </div>

    <div class="form-label">Rclone path</div>
    <div class="form-control column">
      <TextField
        type="text"
        value={settings.rclonePath}
        placeholder="auto-detect if empty"
        on:input={(e) => {
          const next = (e.currentTarget as HTMLInputElement).value
          onPatch({ rclonePath: next })
        }}
        on:blur={() => void onChangeRclonePath(settings.rclonePath)}
      />
      <small>Leave empty to auto-detect `rclone` from the system.</small>
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

<style>
  .cloud-setup-card {
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cloud-setup-headline {
    color: var(--fg-strong);
    font-weight: 700;
  }

  .cloud-setup-next-step {
    color: var(--fg);
  }

  .cloud-setup-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    color: var(--fg-muted);
    font-size: 0.95em;
  }

  .cloud-setup-meta-label {
    font-weight: 600;
    color: var(--fg-muted);
  }

  .cloud-setup-remotes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .cloud-setup-remote {
    border: 1px solid var(--border);
    padding: 2px 6px;
    background: var(--bg-raised);
    color: var(--fg);
  }

  .cloud-setup-busy {
    color: var(--fg-muted);
    font-size: 0.95em;
  }

  .cloud-setup-error {
    color: var(--danger);
    font-size: 0.95em;
  }
</style>
