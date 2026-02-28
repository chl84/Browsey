<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'

  export let value = ''
  export let type: HTMLInputAttributes['type'] = 'text'
  export let placeholder = ''
  export let className = ''
  export let variant: 'default' | 'sidebar' = 'default'
  export let disabled = false
  export let readonly = false
  export let autocomplete: HTMLInputAttributes['autocomplete'] = undefined
  export let autocapitalize: HTMLInputAttributes['autocapitalize'] = undefined
  export let autocorrect: HTMLInputAttributes['autocorrect'] = undefined
  export let spellcheck: boolean | undefined = undefined
  export let name: string | undefined = undefined
  export let id: string | undefined = undefined
  export let inputmode: HTMLInputAttributes['inputmode'] = undefined
  export let required = false
  export let element: HTMLInputElement | null = null
</script>

<input
  {...$$restProps}
  bind:this={element}
  bind:value
  {type}
  {placeholder}
  {disabled}
  {readonly}
  {autocomplete}
  {autocapitalize}
  {autocorrect}
  {spellcheck}
  {name}
  {id}
  {inputmode}
  {required}
  class={`text-field ${variant} ${className}`.trim()}
  on:input
  on:change
  on:focus
  on:blur
  on:keydown
  on:keyup />

<style>
  .text-field {
    width: 100%;
    min-width: 0;
    padding: var(--modal-input-padding-y) var(--modal-input-padding-x);
    border: 1px solid var(--border-accent);
    background: var(--bg);
    color: var(--fg);
    border-radius: 0;
    font-size: var(--modal-font-size);
    line-height: var(--modal-line-height);
    min-height: var(--modal-input-min-height);
  }

  .text-field::placeholder {
    color: var(--fg-dim);
  }

  .text-field:focus {
    border-color: var(--border-accent);
    background: var(--bg-raised);
  }

  .text-field:focus-visible {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }

  .text-field:disabled,
  .text-field:read-only {
    opacity: 0.7;
  }

  .text-field.sidebar {
    min-height: var(--sidebar-row-height);
    padding: 6px 10px;
    font-size: var(--font-size-base);
    line-height: 1.25;
    border-color: var(--border);
    background: var(--bg);
  }

  .text-field.sidebar:focus {
    border-color: var(--border-accent-strong);
    background: var(--bg-raised);
  }

  .text-field::-webkit-search-cancel-button,
  .text-field::-webkit-search-decoration,
  .text-field::-webkit-search-results-button,
  .text-field::-webkit-search-results-decoration {
    cursor: default;
  }
</style>
