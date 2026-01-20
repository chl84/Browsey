<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import type { Entry } from '../types'
  export let open = false
  export let entry: Entry | null = null
  export let count = 1
  export let size: number | null = null
  export let deepCount: number | null = null
  export let onClose: () => void = () => {}
  export let formatSize: (size?: number | null) => string = () => ''
  export let permissions:
    | {
        readOnly: boolean
        executableSupported: boolean
        executable: boolean | null
      }
    | null = null
  export let onToggleReadOnly: (next: boolean) => void = () => {}
  export let onToggleExecutable: (next: boolean) => void = () => {}

  const tabLabels = {
    basic: 'Basic',
    extra: 'Extra',
    permissions: 'Permissions',
  } as const

  let activeTab: 'basic' | 'extra' | 'permissions' = 'basic'
  let availableTabs: Array<'basic' | 'extra' | 'permissions'> = ['basic', 'permissions']
  $: availableTabs = count === 1 ? ['basic', 'extra', 'permissions'] : ['basic', 'permissions']
  $: if (!availableTabs.includes(activeTab)) activeTab = 'basic'
</script>

{#if open}
  <ModalShell open={open} onClose={onClose} modalClass="properties-modal">
    <svelte:fragment slot="header">Properties</svelte:fragment>

    <div class="tabs">
      {#each availableTabs as tab}
        <button
          type="button"
          class:selected={activeTab === tab}
          on:click={() => (activeTab = tab)}
        >
          {tabLabels[tab]}
        </button>
      {/each}
    </div>

    {#if activeTab === 'basic'}
      {#if count === 1 && entry}
        <div class="row"><span class="label">Name</span><span class="value">{entry.name}</span></div>
        <div class="row"><span class="label">Type</span><span class="value">{entry.kind}</span></div>
      {/if}

      <div class="row">
        <span class="label">Size</span>
        <span class="value">
          {#if size !== null && size !== undefined}
            {formatSize(size)}{#if deepCount !== null} {' '}({deepCount} {deepCount === 1 ? 'item' : 'items'}){/if}
          {:else}
            —{#if deepCount !== null} {' '}({deepCount} {deepCount === 1 ? 'item' : 'items'}){/if}
          {/if}
        </span>
      </div>

      {#if count === 1 && entry}
        <div class="row">
          <span class="label">Accessed</span>
          <span class="value">{entry.kind === 'file' ? entry.accessed ?? '—' : '—'}</span>
        </div>
        <div class="row"><span class="label">Modified</span><span class="value">{entry.modified ?? '—'}</span></div>
        <div class="row"><span class="label">Created</span><span class="value">{entry.created ?? '—'}</span></div>
      {/if}
    {:else if activeTab === 'extra'}
      <div class="row"><span class="label">Extra</span><span class="value">Coming soon</span></div>
    {:else if activeTab === 'permissions'}
      {#if count === 1 && entry && permissions}
        <div class="row">
          <span class="label">Read-only</span>
          <span class="value">
            <input
              type="checkbox"
              checked={permissions.readOnly}
              on:change={(e) => onToggleReadOnly((e.currentTarget as HTMLInputElement).checked)}
            />
          </span>
        </div>
        {#if permissions.executableSupported}
          <div class="row">
            <span class="label">Executable</span>
            <span class="value">
              <input
                type="checkbox"
                checked={permissions.executable ?? false}
                on:change={(e) => onToggleExecutable((e.currentTarget as HTMLInputElement).checked)}
              />
            </span>
          </div>
        {/if}
      {:else}
        <div class="row"><span class="label">Permissions</span><span class="value">Coming soon</span></div>
      {/if}
    {/if}

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 12px;
  }

  .tabs button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 6px 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .tabs button.selected {
    background: var(--bg-raised);
    border-color: var(--border-accent);
  }
</style>
