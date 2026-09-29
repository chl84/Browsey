<script lang="ts">
  export let checked = false
  export let disabled = false
  export let name: string
  export let value: string
  export let id: string | undefined = undefined
  export let ariaLabel: string | undefined = undefined
  export let ariaDescribedby: string | undefined = undefined
  export let required = false
  export let element: HTMLInputElement | null = null
</script>

<!-- Keep a native, named radio input for grouping and arrow-key navigation.
     The visible indicator shares Checkbox's sizing and theme tokens. -->
<label class="radio-field" class:disabled>
  <span class="control">
    <input
      {...$$restProps}
      bind:this={element}
      class="native"
      type="radio"
      {checked}
      {disabled}
      {name}
      {value}
      {id}
      {required}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedby}
      on:change
      on:input
      on:focus
      on:blur
      on:keydown
      on:keyup />
    <span class="indicator" aria-hidden="true"><span class="mark"></span></span>
  </span>
  <span class="label-text"><slot /></span>
</label>

<style>
  .radio-field {
    display: inline-grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    column-gap: var(--checkbox-gap, 8px);
    color: var(--fg);
    cursor: default;
    min-width: 0;
    line-height: 1.4;
  }

  .radio-field.disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .control {
    position: relative;
    display: inline-grid;
    width: var(--checkbox-indicator-size, 13px);
    height: var(--checkbox-indicator-size, 13px);
    margin-top: calc((1.4em - var(--checkbox-indicator-size, 13px)) / 2);
  }

  .native {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: inherit;
  }

  .indicator {
    display: inline-grid;
    place-content: center;
    width: var(--checkbox-indicator-size, 13px);
    height: var(--checkbox-indicator-size, 13px);
    border: 1px solid var(--border-accent);
    border-radius: 50%;
    background: var(--bg);
  }

  .mark {
    width: var(--checkbox-indicator-mark-size, 7px);
    height: var(--checkbox-indicator-mark-size, 7px);
    border-radius: 50%;
    background: var(--fg);
    transform: scale(0);
    transition: transform 120ms ease;
  }

  .native:checked + .indicator .mark {
    transform: scale(1);
  }

  .native:focus-visible + .indicator {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }

  .native:disabled + .indicator {
    opacity: 0.75;
  }

  .label-text {
    min-width: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .mark { transition: none; }
  }
</style>
