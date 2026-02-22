<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import type { HTMLInputAttributes } from 'svelte/elements'

  export let value = 0
  export let min: number | string = 0
  export let max: number | string = 100
  export let step: number | string = 1
  export let disabled = false
  export let id: string | undefined = undefined
  export let name: string | undefined = undefined
  export let autocomplete: HTMLInputAttributes['autocomplete'] = undefined
  export let ariaLabel: string | undefined = undefined

  const dispatch = createEventDispatcher<{
    input: { value: number; originalEvent: Event }
    change: { value: number; originalEvent: Event }
  }>()

  const readValue = (event: Event): number => {
    const target = event.currentTarget as HTMLInputElement
    return Number(target.value)
  }

  const handleInput = (event: Event) => {
    const next = readValue(event)
    value = next
    dispatch('input', { value: next, originalEvent: event })
  }

  const handleChange = (event: Event) => {
    const next = readValue(event)
    value = next
    dispatch('change', { value: next, originalEvent: event })
  }
</script>

<input
  class="slider"
  type="range"
  {id}
  {name}
  {autocomplete}
  aria-label={ariaLabel}
  {min}
  {max}
  {step}
  {disabled}
  bind:value
  on:input={handleInput}
  on:change={handleChange}
/>

<style>
  input.slider {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    min-width: 0;
    min-height: 14px;
    height: 14px;
    padding: 0;
    background: transparent;
    border: 0;
    box-shadow: none;
    cursor: pointer;
  }

  input.slider:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  input.slider:focus,
  input.slider:focus-visible {
    outline: none;
    background: transparent;
    border: 0;
    box-shadow: none;
  }

  input.slider::-webkit-slider-runnable-track {
    height: 6px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 0;
  }

  input.slider::-moz-range-track {
    height: 6px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 0;
  }

  input.slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -5px;
    background: var(--bg);
    border: 1px solid var(--border-accent);
    border-radius: 0;
    box-shadow: none;
  }

  input.slider::-moz-range-thumb {
    width: 14px;
    height: 14px;
    background: var(--bg);
    border: 1px solid var(--border-accent);
    border-radius: 0;
    box-shadow: none;
  }

  input.slider:focus-visible::-webkit-slider-thumb {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }

  input.slider:focus-visible::-moz-range-thumb {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }

  input.slider:disabled::-webkit-slider-thumb,
  input.slider:disabled::-moz-range-thumb {
    border-color: var(--border);
  }
</style>
