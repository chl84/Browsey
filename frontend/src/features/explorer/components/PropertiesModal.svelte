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
  type Access = { read: boolean | 'mixed'; write: boolean | 'mixed'; exec: boolean | 'mixed' }
  type HiddenBit = boolean | 'mixed' | null
  const scopes = ['owner', 'group', 'other'] as const
  type Scope = (typeof scopes)[number]
  export let permissions:
    | {
        accessSupported: boolean
        executableSupported: boolean
        readOnly: boolean | 'mixed' | null
        executable: boolean | 'mixed' | null
        owner: Access | null
        group: Access | null
        other: Access | null
      }
    | null = null
  export let hidden: HiddenBit = null
  export let onToggleAccess: (
    scope: Scope,
    key: 'read' | 'write' | 'exec',
    next: boolean
  ) => void = () => {}
  export let onToggleHidden: (next: boolean) => void = () => {}

  const indeterminate = (node: HTMLInputElement, value: boolean | 'mixed' | null | undefined) => {
    node.indeterminate = value === 'mixed'
    return {
      update(next: boolean | 'mixed' | null | undefined) {
        node.indeterminate = next === 'mixed'
      },
    }
  }

  const tabLabels = {
    basic: 'Basic',
    extra: 'Extra',
    permissions: 'Permissions',
  } as const
  const accessLabels: Record<Scope, string> = {
    owner: 'Owner',
    group: 'Group',
    other: 'Other users',
  }

  $: hiddenBit =
    hidden !== null ? hidden : entry ? (entry.hidden === true || entry.name.startsWith('.')) : false

  let activeTab: 'basic' | 'extra' | 'permissions' = 'basic'
  let availableTabs: Array<'basic' | 'extra' | 'permissions'> = ['basic', 'extra', 'permissions']
  let wasOpen = false
  $: availableTabs = ['basic', 'extra', 'permissions']
  $: if (!availableTabs.includes(activeTab)) activeTab = 'basic'
  $: {
    if (open && !wasOpen) {
      activeTab = 'basic'
    }
    wasOpen = open
  }
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
      <div class="row">
        <span class="label">Hidden</span>
        <span class="value">
          <label class="toggle">
            <input
              type="checkbox"
              use:indeterminate={hiddenBit}
              checked={hiddenBit === true}
              title="Hidden attribute"
              on:change={(e) => onToggleHidden((e.currentTarget as HTMLInputElement).checked)}
            />
          </label>
        </span>
      </div>
      <div class="row"><span class="label">Extra</span><span class="value">More coming soon</span></div>
    {:else if activeTab === 'permissions'}
      {#if permissions && permissions.accessSupported}
        <div class="access">
          <div class="row access-head">
            <span class="label"></span>
            <span class="value access-cols">
              <span>Read</span>
              <span>Write</span>
              <span>Exec</span>
            </span>
          </div>
          {#each scopes as scope (scope)}
            {#if permissions[scope]}
              <div class="row access-row">
                <span class="label">{accessLabels[scope]}</span>
                <span class="value access-cols">
                  <label>
                    <input
                      type="checkbox"
                      use:indeterminate={permissions[scope].read}
                      checked={permissions[scope].read === true}
                      on:change={(e) =>
                        onToggleAccess(scope, 'read', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      use:indeterminate={permissions[scope].write}
                      checked={permissions[scope].write === true}
                      on:change={(e) =>
                        onToggleAccess(scope, 'write', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      use:indeterminate={permissions[scope].exec}
                      checked={permissions[scope].exec === true}
                      on:change={(e) =>
                        onToggleAccess(scope, 'exec', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                </span>
              </div>
            {/if}
          {/each}
        </div>
      {:else}
        <div class="row"><span class="label">Permissions</span><span class="value">Not available</span></div>
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

  .access {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .access-head {
    font-weight: 600;
  }

  .access-cols {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    align-items: center;
  }

  .access-row input {
    transform: translateY(1px);
  }
</style>
