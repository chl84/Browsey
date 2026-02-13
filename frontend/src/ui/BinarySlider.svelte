<script lang="ts">
  export let checked = false
  export let leftLabel = 'Off'
  export let rightLabel = 'On'
  export let ariaLabel = 'Toggle'
  export let disabled = false
  export let onToggle: (next: boolean) => void = () => {}

  const toggle = () => {
    if (disabled) return
    onToggle(!checked)
  }
</script>

<button
  class="binary-slider"
  class:checked
  class:disabled
  type="button"
  role="switch"
  aria-label={ariaLabel}
  aria-checked={checked}
  disabled={disabled}
  on:click={toggle}
>
  <span class="labels" aria-hidden="true">
    <span class="label left">{leftLabel}</span>
    <span class="label right">{rightLabel}</span>
  </span>
  <span class="thumb" aria-hidden="true"></span>
</button>

<style>
  .binary-slider {
    --slider-width: var(--binary-slider-width, 74px);
    --slider-height: var(--binary-slider-height, 22px);
    --slider-padding: var(--binary-slider-padding, 1px);
    --slider-thumb-width: var(--binary-slider-thumb-width, 34px);
    --slider-thumb-height: var(--binary-slider-thumb-height, 18px);
    --slider-label-size: var(--binary-slider-label-size, 10px);
    --slider-shift: calc(var(--slider-width) - var(--slider-thumb-width) - var(--slider-padding) * 2);
    position: relative;
    width: var(--slider-width);
    min-width: var(--slider-width);
    height: var(--slider-height);
    flex: 0 0 var(--slider-width);
    flex-shrink: 0;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg-muted);
    border-radius: 0;
    padding: 0;
    cursor: default;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    transition: border-color 140ms ease, background 140ms ease, color 140ms ease;
  }

  .binary-slider:hover:not(.disabled) {
    background: var(--bg-hover);
    border-color: var(--border-accent);
  }

  .binary-slider:focus-visible {
    outline: none;
    border-color: var(--border-accent-strong);
    box-shadow: 0 0 0 var(--focus-ring-width) var(--focus-ring-color);
  }

  .labels {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: center;
    font-size: var(--slider-label-size);
    line-height: 1;
    font-weight: 600;
    letter-spacing: 0.01em;
    text-transform: uppercase;
    pointer-events: none;
  }

  .label {
    text-align: center;
    opacity: 0.45;
    transition: opacity 150ms ease, color 150ms ease;
  }

  .binary-slider:not(.checked) .label.left {
    opacity: 0.95;
    color: var(--fg);
  }

  .binary-slider.checked .label.right {
    opacity: 0.95;
    color: var(--fg);
  }

  .thumb {
    position: absolute;
    top: var(--slider-padding);
    left: var(--slider-padding);
    width: var(--slider-thumb-width);
    height: var(--slider-thumb-height);
    border: 1px solid var(--border-accent);
    background: var(--bg-raised);
    border-radius: 0;
    transform: translateX(0);
    transition: transform 220ms cubic-bezier(0.22, 1, 0.36, 1), border-color 150ms ease, background 150ms ease;
  }

  .binary-slider.checked .thumb {
    transform: translateX(var(--slider-shift));
    border-color: var(--border-accent-strong);
  }

  .binary-slider.disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-reduced-motion: reduce) {
    .binary-slider,
    .label,
    .thumb {
      transition: none;
    }
  }
</style>
