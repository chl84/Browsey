<script lang="ts">
  export let width = 74
  export let height = 10
</script>

<div
  class="pulse-travel"
  aria-hidden="true"
  style={`--pti-width:${width}px; --pti-height:${height}px;`}
>
  <div class="track"></div>
  <div class="orb"></div>
</div>

<style>
  .pulse-travel {
    position: relative;
    width: var(--pti-width);
    height: var(--pti-height);
    display: inline-flex;
    align-items: center;
  }

  .track {
    position: absolute;
    inset: 0;
    background: color-mix(in srgb, var(--border) 70%, transparent);
    opacity: 0.9;
  }

  .orb {
    position: absolute;
    top: 50%;
    left: 0;
    width: var(--pti-height);
    height: var(--pti-height);
    background:
      radial-gradient(circle at 35% 35%, color-mix(in srgb, white 65%, var(--border-accent)) 0 32%, transparent 36%),
      var(--border-accent);
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--border-accent) 55%, black),
      0 0 14px color-mix(in srgb, var(--border-accent) 38%, transparent);
    transform: translate(0, -50%);
    animation:
      pti-travel 1200ms ease-in-out infinite alternate,
      pti-pulse 1200ms ease-in-out infinite;
    will-change: transform, opacity, box-shadow;
  }

  @keyframes pti-travel {
    from {
      transform: translate(0, -50%);
    }
    to {
      transform: translate(calc(var(--pti-width) - var(--pti-height)), -50%);
    }
  }

  @keyframes pti-pulse {
    0% {
      opacity: 0.72;
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--border-accent) 45%, black),
        0 0 8px color-mix(in srgb, var(--border-accent) 20%, transparent);
    }
    50% {
      opacity: 1;
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--border-accent) 65%, black),
        0 0 16px color-mix(in srgb, var(--border-accent) 42%, transparent);
    }
    100% {
      opacity: 0.72;
      box-shadow:
        0 0 0 1px color-mix(in srgb, var(--border-accent) 45%, black),
        0 0 8px color-mix(in srgb, var(--border-accent) 20%, transparent);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .orb {
      animation: pti-pulse 1600ms ease-in-out infinite;
      left: calc((var(--pti-width) - var(--pti-height)) / 2);
      transform: translate(0, -50%);
    }
  }
</style>
