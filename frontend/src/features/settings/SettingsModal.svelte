<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'

  export let open = false
  export let onClose: () => void

  const tabs = [
    { id: 'general', label: 'General' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'archives', label: 'Archives' },
    { id: 'thumbnails', label: 'Thumbnails' },
    { id: 'shortcuts', label: 'Shortcuts' },
    { id: 'performance', label: 'Performance' },
    { id: 'advanced', label: 'Advanced' },
  ] as const

  type TabId = (typeof tabs)[number]['id']
  let activeTab: TabId = 'general'
  let wasOpen = false

  $: {
    if (open && !wasOpen) {
      activeTab = 'general'
    }
    wasOpen = open
  }
</script>

{#if open}
  <ModalShell title="Settings" open={open} onClose={onClose} modalClass="settings-modal">
    <div class="tabs">
      {#each tabs as tab}
        <button
          type="button"
          class:selected={activeTab === tab.id}
          on:click={() => (activeTab = tab.id)}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    {#if activeTab === 'general'}
      <section class="section">
        <h3>General</h3>
        <p>Default view mode, show hidden files, confirmations — placeholder.</p>
      </section>
    {:else if activeTab === 'appearance'}
      <section class="section">
        <h3>Appearance</h3>
        <p>Theme, density, icon sizes — placeholder.</p>
      </section>
    {:else if activeTab === 'archives'}
      <section class="section">
        <h3>Archives</h3>
        <p>Compression defaults, batch behaviour, RAR/7z notes — placeholder.</p>
      </section>
    {:else if activeTab === 'thumbnails'}
      <section class="section">
        <h3>Thumbnails</h3>
        <p>Video thumbnails, cache limits, ffmpeg path — placeholder.</p>
      </section>
    {:else if activeTab === 'shortcuts'}
      <section class="section">
        <h3>Shortcuts</h3>
        <p>View and tweak keybindings — placeholder.</p>
      </section>
    {:else if activeTab === 'performance'}
      <section class="section">
        <h3>Performance</h3>
        <p>Watcher polling, IO concurrency, large folder handling — placeholder.</p>
      </section>
    {:else if activeTab === 'advanced'}
      <section class="section">
        <h3>Advanced</h3>
        <p>External tool paths, logging, experimental flags — placeholder.</p>
      </section>
    {/if}

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Inherits global modal styles; light tweaks for tabs and spacing */
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 12px;
    flex-wrap: wrap;
  }

  .tabs button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 6px 10px;
    font-size: 12px;
    cursor: pointer;
    border-radius: 4px;
  }

  .tabs button.selected {
    background: var(--accent-bg, var(--focus));
    color: var(--accent-fg, var(--fg-strong));
    border-color: var(--accent-border, var(--focus));
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section h3 {
    margin: 0;
    font-size: 14px;
  }

  .section p {
    margin: 0;
    color: var(--fg-dim, #888);
    font-size: 13px;
  }
</style>
