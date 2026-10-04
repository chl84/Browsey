<script context="module" lang="ts">
  let nextDescriptionId = 0
</script>

<script lang="ts">
  import ProgressBar from '../../../shared/ui/ProgressBar.svelte'
  import { partitionCapacity } from '../helpers/partitionCapacity'
  import { fullNameTooltip } from '../helpers/fullNameTooltip'
  import type { VolumeUsageState } from '../modals/propertiesModal'

  export let usage: VolumeUsageState = { data: null, loading: false, error: null }
  export let capacity: number | null | undefined = undefined
  export let unmounted = false
  const descriptionId = `volume-usage-description-${++nextDescriptionId}`

  const formatCapacity = (bytes: number) => bytes === 0 ? '0 GB' : partitionCapacity(bytes)

  $: data = usage.data
  $: percent = data && data.totalBytes > 0 ? (data.usedBytes / data.totalBytes) * 100 : 0
  $: reserved = data && data.reservedBytes > 0
    ? `${partitionCapacity(data.reservedBytes)} is reserved by the filesystem and excluded from free space.`
    : ''
</script>

<section class="volume-usage" aria-label="Storage usage" aria-busy={usage.loading} aria-describedby={reserved ? descriptionId : undefined}>
  {#if usage.loading}
    <p class="hint" role="status">Reading storage usage…</p>
  {:else if data && data.totalBytes > 0}
    <ProgressBar
      role="meter"
      label="Used disk space"
      height="6px"
      fillColor="var(--fg-muted)"
      {percent}
      valueText={`${formatCapacity(data.usedBytes)} used of ${formatCapacity(data.totalBytes)}`}
    />
    <div class="legend" use:fullNameTooltip={() => reserved}>
      <span>{formatCapacity(data.totalBytes)} total</span>
      <span><i class="used" aria-hidden="true"></i>{formatCapacity(data.usedBytes)} used</span>
      <span><i class="free" aria-hidden="true"></i>{formatCapacity(data.freeBytes)} free</span>
    </div>
    {#if reserved}<span class="description" id={descriptionId}>{reserved}</span>{/if}
  {:else}
    {#if partitionCapacity(capacity)}
      <p class="capacity">{partitionCapacity(capacity)} total</p>
    {/if}
    <p class="hint" role="status">
      {usage.error || (unmounted ? 'Usage is available after mounting this volume.' : 'Storage usage is unavailable for this location.')}
    </p>
  {/if}
</section>

<style>
  .volume-usage {
    margin-top: var(--properties-section-margin-top);
    padding-top: 12px;
    min-width: 0;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 6px 12px;
    margin-top: 8px;
    color: var(--fg-muted);
    font-size: var(--properties-ownership-meta-font-size);
  }

  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }

  .legend i {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .used { background: var(--fg-muted); }
  .free { background: var(--border); }
  .hint, .capacity {
    margin: 0;
    color: var(--fg-muted);
    font-size: var(--properties-ownership-meta-font-size);
    overflow-wrap: anywhere;
  }
  .capacity + .hint { margin-top: 4px; }
  .description {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
