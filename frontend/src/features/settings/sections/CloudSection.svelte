<script lang="ts">
  import type { CloudRemoteProbeStatus, CloudSetupStatus } from '@/features/network'
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import Checkbox from '../../../shared/ui/Checkbox.svelte'
  import {
    describeCloudProbeRecommendation,
    describeCloudProbeState,
    describeCloudSetupStatus,
  } from '../cloudSetup'
  import TextField from '../../../shared/ui/TextField.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showCloudEnabledRow = false
  export let showRclonePathRow = false
  export let settings: Settings
  export let cloudSetupStatus: CloudSetupStatus | null = null
  export let cloudSetupStatusBusy = false
  export let cloudSetupStatusError = ''
  export let selectedProbeRemoteId = ''
  export let cloudProbeStatus: CloudRemoteProbeStatus | null = null
  export let cloudProbeBusy = false
  export let cloudProbeError = ''
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onToggleCloudEnabled: (value: boolean) => Promise<void> | void = () => {}
  export let onChangeRclonePath: (value: string) => Promise<void> | void = () => {}
  export let onSelectProbeRemote: (value: string) => void = () => {}
  export let onRunCloudProbe: () => Promise<void> | void = () => {}
  $: setupCopy = describeCloudSetupStatus(cloudSetupStatus)
  $: probeCopy = describeCloudProbeRecommendation(cloudProbeStatus)
  $: probeOptions =
    (cloudSetupStatus?.supportedRemotes ?? []).map((remote) => ({
      value: remote.id,
      label: remote.label,
    })) satisfies ComboOption[]
  $: canProbe =
    settings.cloudEnabled &&
    cloudSetupStatus?.state === 'ready' &&
    probeOptions.length > 0 &&
    selectedProbeRemoteId.length > 0
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

    {#if settings.cloudEnabled && cloudSetupStatus?.state === 'ready' && probeOptions.length > 0}
      <div class="form-label">Test cloud connection</div>
      <div class="form-control column">
        <ComboBox
          value={selectedProbeRemoteId}
          options={probeOptions}
          on:change={(e) => onSelectProbeRemote(e.detail)}
        />
        <button type="button" class="secondary" disabled={!canProbe || cloudProbeBusy} on:click={() => void onRunCloudProbe()}>
          {cloudProbeBusy ? 'Testing...' : 'Test connection'}
        </button>
      </div>

      <div class="form-label">Probe result</div>
      <div class="form-control">
        <div class:disabled={!settings.cloudEnabled} class="cloud-setup-card" aria-live="polite">
          <div class="cloud-setup-headline">{probeCopy.headline}</div>
          <div class="cloud-setup-next-step">{probeCopy.nextStep}</div>

          {#if cloudProbeStatus}
            <div class="cloud-probe-grid">
              <div class="cloud-probe-backend">RC</div>
              <div class:ok={cloudProbeStatus.rc.ok} class="cloud-probe-state">
                {describeCloudProbeState(cloudProbeStatus.rc.ok, cloudProbeStatus.rc.state)}
              </div>
              <div class="cloud-probe-meta">{cloudProbeStatus.rc.elapsedMs} ms</div>
              <div class="cloud-probe-message">{cloudProbeStatus.rc.message}</div>

              <div class="cloud-probe-backend">CLI</div>
              <div class:ok={cloudProbeStatus.cli.ok} class="cloud-probe-state">
                {describeCloudProbeState(cloudProbeStatus.cli.ok, cloudProbeStatus.cli.state)}
              </div>
              <div class="cloud-probe-meta">{cloudProbeStatus.cli.elapsedMs} ms</div>
              <div class="cloud-probe-message">{cloudProbeStatus.cli.message}</div>
            </div>
          {/if}

          {#if cloudProbeError}
            <div class="cloud-setup-error">{cloudProbeError}</div>
          {/if}
        </div>
      </div>
    {/if}
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

  .cloud-probe-grid {
    display: grid;
    grid-template-columns: 48px minmax(110px, auto) auto;
    gap: 6px 10px;
    align-items: baseline;
  }

  .cloud-probe-backend {
    font-weight: 700;
    color: var(--fg-strong);
  }

  .cloud-probe-state {
    color: var(--danger);
    font-weight: 600;
  }

  .cloud-probe-state.ok {
    color: var(--success);
  }

  .cloud-probe-meta {
    color: var(--fg-muted);
    font-size: 0.95em;
  }

  .cloud-probe-message {
    grid-column: 2 / -1;
    color: var(--fg);
    margin-bottom: 2px;
  }
</style>
