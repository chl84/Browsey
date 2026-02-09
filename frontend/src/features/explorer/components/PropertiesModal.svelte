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
  type ExtraField = { key: string; label: string; value: string }
  type ExtraSection = { id: string; title: string; fields: ExtraField[] }
  export let extraMetadataLoading = false
  export let extraMetadataError: string | null = null
  export let extraMetadata: { kind: string; sections: ExtraSection[] } | null = null
  export let permissionsLoading = false
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
        ownerName: string | null
        groupName: string | null
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
  export let onActivateExtra: () => void = () => {}

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
  const principalLabel = (value: string | null | undefined) => {
    if (!value) return '—'
    return value === 'mixed' ? 'Mixed' : value
  }

  $: hiddenBit =
    hidden !== null ? hidden : entry ? (entry.hidden === true || entry.name.startsWith('.')) : false

  let activeTab: 'basic' | 'extra' | 'permissions' = 'basic'
  let availableTabs: Array<'basic' | 'extra' | 'permissions'> = ['basic', 'extra', 'permissions']
  let wasOpen = false
  const switchTab = (tab: 'basic' | 'extra' | 'permissions') => {
    activeTab = tab
    if (tab === 'extra') onActivateExtra()
  }
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
          on:click={() => switchTab(tab)}
        >
          {tabLabels[tab]}
        </button>
      {/each}
    </div>

    {#if activeTab === 'basic'}
      <div class="rows">
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
      </div>
    {:else if activeTab === 'extra'}
      {#if count !== 1}
        <div class="rows status-rows">
          <div class="row"><span class="label">Extra</span><span class="value">Select one item to view extra metadata</span></div>
        </div>
      {:else if extraMetadataLoading}
        <div class="rows status-rows">
          <div class="row"><span class="label">Extra</span><span class="value">Loading…</span></div>
        </div>
      {:else if extraMetadataError}
        <div class="rows status-rows">
          <div class="row"><span class="label">Extra</span><span class="value">Failed to load: {extraMetadataError}</span></div>
        </div>
      {:else if extraMetadata && extraMetadata.sections.length > 0}
        {#each extraMetadata.sections as section (section.id)}
          <div class="section extra-section">
            <div class="rows">
              {#each section.fields as field (field.key)}
                <div class="row">
                  <span class="label">{field.label}</span>
                  <span class="value">{field.value || '—'}</span>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      {:else}
        <div class="rows status-rows">
          <div class="row"><span class="label">Extra</span><span class="value">No extra metadata available</span></div>
        </div>
      {/if}
    {:else if activeTab === 'permissions'}
      {#if permissionsLoading}
        <div class="rows status-rows">
          <div class="row"><span class="label">Permissions</span><span class="value">Loading…</span></div>
        </div>
      {:else if permissions}
        <div class="rows ownership">
          <div class="row"><span class="label">User</span><span class="value">{principalLabel(permissions.ownerName)}</span></div>
          <div class="row"><span class="label">Group</span><span class="value">{principalLabel(permissions.groupName)}</span></div>
        </div>

        {#if permissions.accessSupported}
          <div class="access">
            <div class="cell head"></div>
            <div class="cell head">Read</div>
            <div class="cell head">Write</div>
            <div class="cell head">Exec</div>
            {#each scopes as scope (scope)}
              {#if permissions[scope]}
                <div class="cell label">{accessLabels[scope]}</div>
                <label class="cell">
                  <input
                    type="checkbox"
                    use:indeterminate={permissions[scope].read}
                    checked={permissions[scope].read === true}
                    on:change={(e) =>
                      onToggleAccess(scope, 'read', (e.currentTarget as HTMLInputElement).checked)}
                  />
                </label>
                <label class="cell">
                  <input
                    type="checkbox"
                    use:indeterminate={permissions[scope].write}
                    checked={permissions[scope].write === true}
                    on:change={(e) =>
                      onToggleAccess(scope, 'write', (e.currentTarget as HTMLInputElement).checked)}
                  />
                </label>
                <label class="cell">
                  <input
                    type="checkbox"
                    use:indeterminate={permissions[scope].exec}
                    checked={permissions[scope].exec === true}
                    on:change={(e) =>
                      onToggleAccess(scope, 'exec', (e.currentTarget as HTMLInputElement).checked)}
                  />
                </label>
              {/if}
            {/each}
          </div>
        {:else}
          <div class="rows status-rows">
            <div class="row">
              <span class="label">Permissions</span>
              <span class="value">Not available for one or more selected items</span>
            </div>
          </div>
        {/if}
      {:else}
        <div class="rows status-rows">
          <div class="row"><span class="label">Permissions</span><span class="value">Not available</span></div>
        </div>
      {/if}
    {/if}

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
  .rows {
    display: grid;
    grid-template-columns: 120px 1fr;
    row-gap: 8px;
    column-gap: 10px;
  }

  .row {
    display: contents;
  }

  .label {
    color: var(--fg-muted);
    font-weight: 600;
    text-align: right;
  }

  .value {
    overflow-wrap: anywhere;
  }

  .access {
    margin-top: 8px;
    display: grid;
    grid-template-columns: 120px repeat(3, minmax(70px, 1fr));
    row-gap: 8px;
    column-gap: 10px;
    align-items: center;
    justify-content: start;
    width: max-content;
    transform: translateX(-20px);
  }

  .ownership {
    margin-bottom: 8px;
  }

  .status-rows {
    margin-top: 8px;
  }

  .extra-section + .extra-section {
    margin-top: 8px;
  }

  .access .cell {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: center;
  }

  .access .cell.head {
    font-weight: 600;
    white-space: nowrap;
    justify-content: center;
  }

  .access .cell.label {
    justify-content: flex-end;
    text-align: right;
    white-space: nowrap;
    justify-self: end;
  }
</style>
