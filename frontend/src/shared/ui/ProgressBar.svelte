<script lang="ts">
  export let percent: number | null = null
  export let label = 'Progress'
  export let width = '100%'
  export let height = '8px'

  $: value = percent !== null && Number.isFinite(percent) ? Math.min(100, Math.max(0, percent)) : null
</script>

<div
  class="progress-bar"
  role="progressbar"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="100"
  aria-valuenow={value ?? undefined}
  style={`width:${width};height:${height};`}
>
  <div class="progress-fill" class:indeterminate={value === null} style:width={value === null ? '30%' : `${value}%`}></div>
</div>

<style>
  .progress-bar { background: var(--border); overflow: hidden; flex-shrink: 0; }
  .progress-fill { height: 100%; background: var(--border-accent); transition: width 120ms ease; }
  .indeterminate { animation: travel 1400ms ease-in-out infinite alternate; }
  @keyframes travel { from { transform: translateX(0); } to { transform: translateX(233%); } }
  @media (prefers-reduced-motion: reduce) {
    .indeterminate { animation: none; transform: translateX(116%); }
    .progress-fill { transition: none; }
  }
</style>
