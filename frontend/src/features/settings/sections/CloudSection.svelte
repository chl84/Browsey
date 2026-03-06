<script lang="ts">
  import type { CloudSetupStatus } from '@/features/network'
  import Checkbox from '../../../shared/ui/Checkbox.svelte'
  import { describeCloudSetupStatus } from '../cloudSetup'
  import TextField from '../../../shared/ui/TextField.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showCloudEnabledRow = false
  export let showRclonePathRow = false
  export let settings: Settings
  export let cloudSetupStatus: CloudSetupStatus | null = null
  export let cloudSetupStatusBusy = false
  export let cloudSetupStatusError = ''
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleCloudEnabled: (value: boolean) => Promise<void> | void = () => {}
  export let onChangeRclonePath: (value: string) => Promise<void> | void = () => {}
  $: setupCopy = describeCloudSetupStatus(cloudSetupStatus)
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Cloud</div><div class="group-spacer"></div>

  {#if showCloudEnabledRow}
    <div class="form-label">Enable cloud</div>
    <div class="form-control checkbox">
      <Checkbox
        checked={settings.cloudEnabled}
        on:change={(e) => {
          const next = (e.target as HTMLInputElement).checked
          onPatch({ cloudEnabled: next })
          void onToggleCloudEnabled(next)
        }}
      >
        Enable cloud folders via rclone
      </Checkbox>
    </div>
  {/if}

  {#if showRclonePathRow}
    <div class="form-label">Cloud setup</div>
    <div class="form-control">
      <div class:disabled={!settings.cloudEnabled} class="cloud-setup-card" aria-live="polite">
        {#if settings.cloudEnabled}
          <div class="cloud-setup-headline">{setupCopy.headline}</div>
          <div class="cloud-setup-next-step">{setupCopy.nextStep}</div>
        {:else}
          <div class="cloud-setup-headline">Cloud via rclone is off</div>
          <div class="cloud-setup-next-step">Turn it on to show cloud remotes in Network.</div>
        {/if}

        {#if settings.cloudEnabled && cloudSetupStatus?.resolvedBinaryPath}
          <div class="cloud-setup-meta">
            <span class="cloud-setup-meta-label">Binary</span>
            <code>{cloudSetupStatus.resolvedBinaryPath}</code>
          </div>
        {/if}

        {#if settings.cloudEnabled && cloudSetupStatus}
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

        {#if settings.cloudEnabled && cloudSetupStatus?.state === 'ready' && cloudSetupStatus.supportedRemotes.length > 0}
          <div class="cloud-setup-remotes">
            {#each cloudSetupStatus.supportedRemotes as remote}
              <span class="cloud-setup-remote">{remote.label}</span>
            {/each}
          </div>
        {/if}

        {#if settings.cloudEnabled && cloudSetupStatusBusy}
          <div class="cloud-setup-busy">Refreshing status…</div>
        {/if}

        {#if settings.cloudEnabled && cloudSetupStatusError}
          <div class="cloud-setup-error">{cloudSetupStatusError}</div>
        {/if}
      </div>
    </div>

    <div class="form-label">Rclone path</div>
    <div class="form-control column">
      <TextField
        type="text"
        disabled={!settings.cloudEnabled}
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

  .cloud-setup-card.disabled {
    opacity: 0.75;
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
