<script lang="ts">
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showDefaultViewRow = false
  export let showFoldersFirstRow = false
  export let showShowHiddenRow = false
  export let showHiddenFilesLastRow = false
  export let showStartDirRow = false
  export let showConfirmDeleteRow = false
  export let hiddenFilesLastDisabled = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeDefaultView: (value: 'list' | 'grid') => void = () => {}
  export let onToggleFoldersFirst: (value: boolean) => void = () => {}
  export let onToggleShowHidden: (value: boolean) => void = () => {}
  export let onToggleHiddenFilesLast: (value: boolean) => void = () => {}
  export let onChangeStartDir: (value: string) => void = () => {}
  export let onToggleConfirmDelete: (value: boolean) => void = () => {}
</script>

{#if show}
  <div class="group-heading">General</div><div class="group-spacer"></div>

  {#if showDefaultViewRow}
    <div class="form-label">Default view</div>
    <div class="form-control radios">
      <label class="radio">
        <input
          type="radio"
          name="default-view"
          value="list"
          checked={settings.defaultView === 'list'}
          on:change={() => {
            onPatch({ defaultView: 'list' })
            onChangeDefaultView('list')
          }}
        />
        <span>List</span>
      </label>
      <label class="radio">
        <input
          type="radio"
          name="default-view"
          value="grid"
          checked={settings.defaultView === 'grid'}
          on:change={() => {
            onPatch({ defaultView: 'grid' })
            onChangeDefaultView('grid')
          }}
        />
        <span>Grid</span>
      </label>
    </div>
  {/if}

  {#if showFoldersFirstRow}
    <div class="form-label">Folders first</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.foldersFirst}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ foldersFirst: next })
          onToggleFoldersFirst(next)
        }}
      />
      <span>Show folders before files</span>
    </div>
  {/if}

  {#if showShowHiddenRow}
    <div class="form-label">Show hidden</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.showHidden}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ showHidden: next })
          onToggleShowHidden(next)
        }}
      />
      <span>Show hidden files by default</span>
    </div>
  {/if}

  {#if showHiddenFilesLastRow}
    <div class="form-label">Hidden files last</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.hiddenFilesLast}
        disabled={hiddenFilesLastDisabled}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ hiddenFilesLast: next })
          onToggleHiddenFilesLast(next)
        }}
      />
      <span>Place hidden items at the end</span>
      {#if hiddenFilesLastDisabled}
        <small>Enable "Show hidden" to change this</small>
      {/if}
    </div>
  {/if}

  {#if showStartDirRow}
    <div class="form-label">Start directory</div>
    <div class="form-control">
      <input
        type="text"
        value={settings.startDir}
        placeholder="~ or /path"
        on:input={(e) => {
          const next = (e.currentTarget as HTMLInputElement).value
          onPatch({ startDir: next })
          onChangeStartDir(next)
        }}
      />
    </div>
  {/if}

  {#if showConfirmDeleteRow}
    <div class="form-label">Confirm delete</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.confirmDelete}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ confirmDelete: next })
          onToggleConfirmDelete(next)
        }}
      />
      <span>Ask before permanent delete</span>
    </div>
  {/if}
{/if}
