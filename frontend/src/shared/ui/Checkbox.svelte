<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'
  import CheckboxIndicator from './CheckboxIndicator.svelte'

  export let checked = false
  export let disabled = false
  export let indeterminate = false
  export let name: string | undefined = undefined
  export let id: string | undefined = undefined
  export let value: HTMLInputAttributes['value'] = undefined
  export let ariaLabel: string | undefined = undefined
  export let ariaDescribedby: string | undefined = undefined
  export let required = false
  export let className = ''
  export let inputClassName = ''
  export let element: HTMLInputElement | null = null

  $: if (element) {
    element.indeterminate = indeterminate
  }
</script>

<label class={`checkbox-field ${className}`.trim()} class:disabled>
  <span class="control">
    <input
      {...$$restProps}
      bind:this={element}
      bind:checked
      class={`native ${inputClassName}`.trim()}
      type="checkbox"
      {disabled}
      {name}
      {id}
      {value}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedby}
      {required}
      on:change
      on:input
      on:focus
      on:blur
      on:keydown
      on:keyup />
    <CheckboxIndicator {checked} {indeterminate} />
  </span>
  {#if $$slots.default || $$slots.description}
    <span class="body">
      {#if $$slots.default}
        <span class="label-text"><slot /></span>
      {/if}
      {#if $$slots.description}
        <span class="description"><slot name="description" /></span>
      {/if}
    </span>
  {/if}
</label>

<style>
  .checkbox-field {
    display: inline-grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    column-gap: var(--checkbox-gap, 8px);
    row-gap: 2px;
    color: var(--fg);
    cursor: default;
    min-width: 0;
    line-height: 1.4;
  }

  .checkbox-field.disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .control {
    position: relative;
    display: inline-grid;
    align-items: center;
    justify-items: center;
    width: var(--checkbox-indicator-size, 13px);
    min-width: var(--checkbox-indicator-size, 13px);
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

  .native:focus-visible + :global(.checkbox-indicator) {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }

  .native:disabled + :global(.checkbox-indicator) {
    opacity: 0.75;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .label-text {
    line-height: 1.4;
    min-width: 0;
  }

  .description {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--fg-muted);
    font-size: 12px;
    line-height: 1.35;
  }

  .description :global(small) {
    color: inherit;
    font-size: inherit;
    line-height: inherit;
  }
</style>
