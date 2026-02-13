<script lang="ts">
  import { onMount } from 'svelte'

  let theme: 'light' | 'dark' = 'dark'

  onMount(() => {
    const stored = localStorage.getItem('browsey-theme')
    const current = stored === 'light' ? 'light' : 'dark'
    applyTheme(current)
  })

  const applyTheme = (next: 'light' | 'dark') => {
    theme = next
    const root = document.documentElement
    if (next === 'light') {
      root.dataset.theme = 'light'
    } else {
      root.removeAttribute('data-theme')
    }
    localStorage.setItem('browsey-theme', next)
  }

  const toggleTheme = () => {
    applyTheme(theme === 'dark' ? 'light' : 'dark')
  }
</script>

<button
  class="theme-toggle"
  class:light={theme === 'light'}
  type="button"
  aria-label="Toggle theme"
  aria-pressed={theme === 'light'}
  on:click|stopPropagation={toggleTheme}
>
  <svg class="track-icon sun" viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="12" r="3.5"></circle>
    <path
      d="M12 2.3v3M12 18.7v3M4.4 4.4l2.1 2.1M17.5 17.5l2.1 2.1M2.3 12h3M18.7 12h3M4.4 19.6l2.1-2.1M17.5 6.5l2.1-2.1"
    ></path>
  </svg>
  <svg class="track-icon moon" viewBox="0 0 24 24" aria-hidden="true">
    <path d="M21 12.8A9 9 0 1111.2 3 7 7 0 0021 12.8z"></path>
  </svg>
  <span class="thumb" aria-hidden="true">
    <svg class="thumb-icon sun" viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="3.5"></circle>
      <path
        d="M12 2.3v3M12 18.7v3M4.4 4.4l2.1 2.1M17.5 17.5l2.1 2.1M2.3 12h3M18.7 12h3M4.4 19.6l2.1-2.1M17.5 6.5l2.1-2.1"
      ></path>
    </svg>
    <svg class="thumb-icon moon" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M21 12.8A9 9 0 1111.2 3 7 7 0 0021 12.8z"></path>
    </svg>
  </span>
  <span class="sr-only">{theme === 'light' ? 'Light mode' : 'Dark mode'}</span>
</button>

<style>
  .theme-toggle {
    position: relative;
    width: var(--topbar-theme-toggle-width);
    height: var(--topbar-theme-toggle-height);
    border: 1px solid var(--win-btn-border);
    background: linear-gradient(120deg, var(--bg-raised), var(--bg));
    border-radius: 0;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background 220ms ease, border-color 220ms ease, color 220ms ease;
    color: var(--fg-muted);
    overflow: hidden;
    margin-right: var(--topbar-theme-toggle-margin-right);
  }

  .theme-toggle:focus-visible {
    outline: none;
    border-color: var(--border-accent-strong);
    box-shadow: 0 0 0 var(--focus-ring-width) var(--focus-ring-color);
  }

  .track-icon {
    position: absolute;
    top: 50%;
    width: var(--topbar-theme-toggle-icon-size);
    height: var(--topbar-theme-toggle-icon-size);
    opacity: 0.42;
    transform: translateY(-50%) scale(0.92);
    transition: opacity 220ms ease, transform 220ms ease, color 220ms ease;
    pointer-events: none;
    vector-effect: non-scaling-stroke;
  }

  .track-icon.sun {
    left: var(--topbar-theme-toggle-icon-sun-left);
    fill: none;
    stroke: var(--theme-toggle-icon-sun);
    stroke-width: 1.9;
    stroke-linecap: square;
    stroke-linejoin: miter;
  }

  .track-icon.moon {
    right: var(--topbar-theme-toggle-icon-moon-right);
    fill: var(--theme-toggle-icon-moon);
    stroke: none;
  }

  .theme-toggle.light .track-icon.sun {
    opacity: 0.9;
    transform: translateY(-50%) scale(1);
  }

  .theme-toggle.light .track-icon.moon {
    opacity: 0.26;
    transform: translateY(-50%) scale(0.9);
  }

  .theme-toggle:not(.light) .track-icon.sun {
    opacity: 0.26;
    transform: translateY(-50%) scale(0.9);
  }

  .theme-toggle:not(.light) .track-icon.moon {
    opacity: 0.92;
    transform: translateY(-50%) scale(1);
  }

  .thumb {
    position: absolute;
    top: calc((100% - var(--topbar-theme-toggle-thumb-height)) / 2);
    left: var(--topbar-theme-toggle-inset);
    width: var(--topbar-theme-toggle-thumb-width);
    height: var(--topbar-theme-toggle-thumb-height);
    border-radius: 0;
    background: var(--theme-toggle-thumb-light);
    border: 1px solid var(--win-btn-border);
    color: var(--theme-toggle-icon-sun);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transform: translateX(0);
    transition:
      transform 300ms cubic-bezier(0.22, 1, 0.36, 1),
      background 220ms ease,
      color 220ms ease,
      border-color 220ms ease;
  }

  .theme-toggle:not(.light) .thumb {
    transform: translateX(var(--topbar-theme-toggle-thumb-offset));
    background: linear-gradient(
      135deg,
      var(--theme-toggle-thumb-dark-start),
      var(--theme-toggle-thumb-dark-end)
    );
    border-color: var(--border-accent);
    color: var(--theme-toggle-icon-moon);
  }

  .thumb-icon {
    position: absolute;
    width: calc(var(--topbar-theme-toggle-icon-size) - 1px);
    height: calc(var(--topbar-theme-toggle-icon-size) - 1px);
    transition:
      opacity 210ms ease,
      transform 260ms cubic-bezier(0.22, 1, 0.36, 1);
    pointer-events: none;
  }

  .thumb-icon.sun {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: square;
    stroke-linejoin: miter;
  }

  .thumb-icon.moon {
    fill: currentColor;
    stroke: none;
  }

  .theme-toggle.light .thumb-icon.sun {
    opacity: 1;
    transform: scale(1);
  }

  .theme-toggle.light .thumb-icon.moon {
    opacity: 0;
    transform: scale(0.68);
  }

  .theme-toggle:not(.light) .thumb-icon.sun {
    opacity: 0;
    transform: scale(0.68);
  }

  .theme-toggle:not(.light) .thumb-icon.moon {
    opacity: 1;
    transform: scale(1);
  }

  @media (prefers-reduced-motion: reduce) {
    .theme-toggle,
    .track-icon,
    .thumb,
    .thumb-icon {
      transition: none;
    }
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
