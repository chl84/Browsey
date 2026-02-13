<script lang="ts">
  export let checked = false
</script>

<span
  class="checkbox-indicator"
  class:checked
  aria-hidden="true"
>
  <span class="check-mark"></span>
</span>

<style>
  .checkbox-indicator {
    width: var(--checkbox-indicator-size, 13px);
    height: var(--checkbox-indicator-size, 13px);
    border: 1px solid var(--border-accent);
    background: var(--bg);
    display: inline-grid;
    place-content: center;
    flex: 0 0 var(--checkbox-indicator-size, 13px);
    flex-shrink: 0;
  }

  .check-mark {
    width: var(--checkbox-indicator-mark-size, 7px);
    height: var(--checkbox-indicator-mark-size, 7px);
    background: var(--fg);
    transform: scale(0);
    transition: transform 120ms ease;
  }

  .checkbox-indicator.checked .check-mark {
    transform: scale(1);
  }

  @media (prefers-reduced-motion: reduce) {
    .check-mark {
      transition: none;
    }
  }
</style>
