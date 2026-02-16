<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import ComboBox, { type ComboOption } from '../../../ui/ComboBox.svelte'
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
  export let ownershipApplying = false
  export let ownershipError: string | null = null
  export let ownershipUsers: string[] = []
  export let ownershipGroups: string[] = []
  export let ownershipOptionsLoading = false
  export let ownershipOptionsError: string | null = null
  type Access = { read: boolean | 'mixed'; write: boolean | 'mixed'; exec: boolean | 'mixed' }
  type HiddenBit = boolean | 'mixed' | null
  const scopes = ['owner', 'group', 'other'] as const
  type Scope = (typeof scopes)[number]
  export let permissions:
    | {
        accessSupported: boolean
        executableSupported: boolean
        ownershipSupported: boolean
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
  export let mutationsLocked = false
  export let onToggleAccess: (
    scope: Scope,
    key: 'read' | 'write' | 'exec',
    next: boolean
  ) => void = () => {}
  export let onToggleHidden: (next: boolean) => void = () => {}
  export let onActivateExtra: () => void = () => {}
  export let onSetOwnership: (owner: string, group: string) => void | Promise<void> = () => {}

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
  const editablePrincipal = (value: string | null | undefined) => {
    if (!value || value === 'mixed') return ''
    return value
  }
  const principalPlaceholder = (value: string | null | undefined) => {
    if (value === 'mixed') return 'Mixed'
    return value ?? ''
  }
  const toPrincipalOptions = (values: string[]): ComboOption[] =>
    values.map((value) => ({ value, label: value }))
  const ensurePrincipalOption = (
    options: ComboOption[],
    current: string | null | undefined,
    selected: string,
  ): ComboOption[] => {
    const normalizedCurrent = editablePrincipal(current).trim()
    const normalizedSelected = selected.trim()
    const merged = [...options]
    for (const candidate of [normalizedCurrent, normalizedSelected]) {
      if (!candidate) continue
      if (merged.some((option) => option.value === candidate)) continue
      merged.unshift({ value: candidate, label: candidate })
    }
    return merged
  }
  const applyOwnership = () => {
    void onSetOwnership(ownerInput, groupInput)
  }

  $: hiddenBit =
    hidden !== null ? hidden : entry ? (entry.hidden === true || entry.name.startsWith('.')) : false

  let activeTab: 'basic' | 'extra' | 'permissions' = 'basic'
  let availableTabs: Array<'basic' | 'extra' | 'permissions'> = ['basic', 'extra', 'permissions']
  let wasOpen = false
  let ownershipInputsInitialized = false
  let ownerInput = ''
  let groupInput = ''
  $: ownerOptions = ensurePrincipalOption(
    toPrincipalOptions(ownershipUsers),
    permissions?.ownerName,
    ownerInput,
  )
  $: groupOptions = ensurePrincipalOption(
    toPrincipalOptions(ownershipGroups),
    permissions?.groupName,
    groupInput,
  )
  const switchTab = (tab: 'basic' | 'extra' | 'permissions') => {
    activeTab = tab
    if (tab === 'extra') onActivateExtra()
  }
  $: availableTabs = ['basic', 'extra', 'permissions']
  $: if (!availableTabs.includes(activeTab)) activeTab = 'basic'
  $: {
    if (open && !wasOpen) {
      activeTab = 'basic'
      ownershipInputsInitialized = false
      ownerInput = ''
      groupInput = ''
    }
    if (!open && wasOpen) {
      ownershipInputsInitialized = false
      ownerInput = ''
      groupInput = ''
    }
    if (open && permissions && !ownershipInputsInitialized) {
      ownerInput = editablePrincipal(permissions.ownerName)
      groupInput = editablePrincipal(permissions.groupName)
      ownershipInputsInitialized = true
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
                disabled={mutationsLocked}
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
        <div class="permissions-panel">
          <div class="rows ownership">
            {#if permissions.ownershipSupported}
              <div class="row">
                <span class="label">User</span>
                <span class="value">
                  <ComboBox
                    options={ownerOptions}
                    value={ownerInput}
                    placeholder={principalPlaceholder(permissions.ownerName)}
                    searchable={true}
                    searchPlaceholder="Search users"
                    emptyLabel={ownershipOptionsLoading ? 'Loading users…' : 'No users found'}
                    noMatchesLabel="No matching users"
                    disabled={ownershipApplying || mutationsLocked}
                    on:change={(e) => (ownerInput = e.detail as string)}
                  />
                </span>
              </div>
              <div class="row">
                <span class="label">Group</span>
                <span class="value">
                  <ComboBox
                    options={groupOptions}
                    value={groupInput}
                    placeholder={principalPlaceholder(permissions.groupName)}
                    searchable={true}
                    searchPlaceholder="Search groups"
                    emptyLabel={ownershipOptionsLoading ? 'Loading groups…' : 'No groups found'}
                    noMatchesLabel="No matching groups"
                    disabled={ownershipApplying || mutationsLocked}
                    on:change={(e) => (groupInput = e.detail as string)}
                  />
                </span>
              </div>
              <div class="row">
                <span class="label" aria-hidden="true"></span>
                <span class="value ownership-controls">
                  <button
                    type="button"
                    class="ownership-apply-button"
                    on:click={applyOwnership}
                    disabled={ownershipApplying || mutationsLocked}
                  >
                    {ownershipApplying ? 'Applying…' : 'Apply ownership'}
                  </button>
                  {#if ownershipError}
                    <span class="ownership-error">{ownershipError}</span>
                  {:else if ownershipOptionsError}
                    <span class="ownership-hint">Failed to load users/groups: {ownershipOptionsError}</span>
                  {:else if ownershipOptionsLoading}
                    <span class="ownership-hint">Loading users/groups…</span>
                  {/if}
                </span>
              </div>
            {:else}
              <div class="row"><span class="label">User</span><span class="value">{principalLabel(permissions.ownerName)}</span></div>
              <div class="row"><span class="label">Group</span><span class="value">{principalLabel(permissions.groupName)}</span></div>
              <div class="row">
                <span class="label" aria-hidden="true"></span>
                <span class="value ownership-hint">Changing user/group is not supported on this platform.</span>
              </div>
            {/if}
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
                      disabled={mutationsLocked}
                      on:change={(e) =>
                        onToggleAccess(scope, 'read', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                  <label class="cell">
                    <input
                      type="checkbox"
                      use:indeterminate={permissions[scope].write}
                      checked={permissions[scope].write === true}
                      disabled={mutationsLocked}
                      on:change={(e) =>
                        onToggleAccess(scope, 'write', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                  <label class="cell">
                    <input
                      type="checkbox"
                      use:indeterminate={permissions[scope].exec}
                      checked={permissions[scope].exec === true}
                      disabled={mutationsLocked}
                      on:change={(e) =>
                        onToggleAccess(scope, 'exec', (e.currentTarget as HTMLInputElement).checked)}
                    />
                  </label>
                {/if}
              {/each}
            </div>
          {:else}
            <div class="rows status-rows permissions-status">
              <div class="row">
                <span class="label">Permissions</span>
                <span class="value">Not available for one or more selected items</span>
              </div>
            </div>
          {/if}
        </div>
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
    grid-template-columns: var(--properties-label-width) 1fr;
    row-gap: var(--properties-row-gap);
    column-gap: var(--properties-col-gap);
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
    margin-top: 0;
    display: grid;
    grid-template-columns: max-content repeat(3, var(--properties-access-col-width));
    row-gap: var(--properties-access-row-gap);
    column-gap: var(--properties-access-col-gap);
    align-items: center;
    width: max-content;
  }

  .ownership {
    margin: 0;
    width: 100%;
    max-width: 100%;
    grid-template-columns: max-content minmax(0, 1fr);
    row-gap: var(--properties-ownership-row-gap);
    column-gap: var(--properties-ownership-col-gap);
  }

  .permissions-panel {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--properties-panel-gap);
    margin-top: var(--properties-panel-margin-top);
  }

  .ownership .label {
    text-align: left;
    justify-self: start;
    min-width: var(--properties-ownership-label-min-width);
  }

  .ownership .value {
    justify-self: start;
    min-width: 0;
  }

  .ownership :global(.combo) {
    width: var(--properties-ownership-input-width);
    max-width: min(var(--properties-ownership-input-max-width), 100%);
  }

  .ownership :global(.combo-list) {
    max-height: calc(var(--modal-input-min-height) * 5.2);
  }

  .ownership-controls {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--properties-ownership-controls-gap);
    max-width: 100%;
  }

  .ownership-hint {
    color: var(--fg-muted);
    font-size: var(--properties-ownership-meta-font-size);
  }

  .ownership-error {
    color: #c03a2b;
    font-size: var(--properties-ownership-meta-font-size);
    display: block;
    max-width: 100%;
    white-space: normal;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .ownership-apply-button {
    padding: var(--properties-apply-button-padding-y) var(--properties-apply-button-padding-x);
    min-height: var(--properties-apply-button-min-height);
    font-size: var(--properties-apply-button-font-size);
  }

  .permissions-status {
    margin-top: 0;
  }

  .status-rows {
    margin-top: var(--properties-status-margin-top);
  }

  .extra-section + .extra-section {
    margin-top: var(--properties-section-margin-top);
  }

  .access .cell {
    display: flex;
    align-items: center;
    gap: var(--properties-access-cell-gap);
    justify-content: center;
  }

  .access .cell.head {
    font-weight: 600;
    white-space: nowrap;
    justify-content: center;
  }

  .access .cell.label {
    justify-content: flex-start;
    text-align: left;
    white-space: nowrap;
    justify-self: start;
    padding-right: var(--properties-access-label-padding-right);
  }
</style>
