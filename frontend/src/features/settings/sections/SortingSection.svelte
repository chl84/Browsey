<script lang="ts">
  import ComboBox, { type ComboOption } from '../../../shared/ui/ComboBox.svelte'
  import Radio from '../../../shared/ui/Radio.svelte'
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showSortFieldRow = false
  export let showSortDirectionRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeSortField: (value: Settings['sortField']) => void = () => {}
  export let onChangeSortDirection: (value: Settings['sortDirection']) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Sorting</div><div class="group-spacer"></div>

  {#if showSortFieldRow}
    <div class="form-label">Default sort field</div>
    <div class="form-control">
      <ComboBox
        value={settings.sortField}
        on:change={(e) => {
          const next = e.detail as Settings['sortField']
          onPatch({ sortField: next })
          onChangeSortField(next)
        }}
        options={[
          { value: 'name', label: 'Name' },
          { value: 'type', label: 'Type' },
          { value: 'modified', label: 'Date modified' },
          { value: 'size', label: 'Size' },
        ] satisfies ComboOption[]}
      />
    </div>
  {/if}

  {#if showSortDirectionRow}
    <div class="form-label">Sort direction</div>
    <div class="form-control radios">
      <Radio
        name="sort-direction"
        value="asc"
        checked={settings.sortDirection === 'asc'}
        on:change={() => {
          onPatch({ sortDirection: 'asc' })
          onChangeSortDirection('asc')
        }}
      >Ascending</Radio>
      <Radio
        name="sort-direction"
        value="desc"
        checked={settings.sortDirection === 'desc'}
        on:change={() => {
          onPatch({ sortDirection: 'desc' })
          onChangeSortDirection('desc')
        }}
      >Descending</Radio>
    </div>
  {/if}
{/if}
