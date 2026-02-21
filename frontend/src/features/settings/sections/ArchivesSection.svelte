<script lang="ts">
  import type { Settings } from '../settingsTypes'

  export let show = false
  export let showArchiveNameRow = false
  export let showArchiveLevelRow = false
  export let showAfterExtractRow = false
  export let showRarNoteRow = false
  export let settings: Settings
  export let onPatch: (patch: Partial<Settings>) => void = () => {}
  export let onChangeArchiveName: (value: string) => void = () => {}
  export let onChangeArchiveLevel: (value: number) => void = () => {}
  export let onToggleOpenDestAfterExtract: (value: boolean) => void = () => {}
</script>

{#if show}
  <div class="group-divider" aria-hidden="true"></div>
  <div class="group-heading">Archives</div><div class="group-spacer"></div>

  {#if showArchiveNameRow}
    <div class="form-label">Default archive name</div>
    <div class="form-control archive-name">
      <input
        type="text"
        value={settings.archiveName}
        on:input={(e) => {
          const val = (e.currentTarget as HTMLInputElement).value
          onPatch({ archiveName: val })
          onChangeArchiveName(val)
        }}
      />
      <span class="suffix">.zip</span>
    </div>
  {/if}

  {#if showArchiveLevelRow}
    <div class="form-label">ZIP level</div>
    <div class="form-control">
      <input
        type="range"
        min="0"
        max="9"
        step="1"
        value={settings.archiveLevel}
        on:input={(e) => {
          const next = Number((e.currentTarget as HTMLInputElement).value)
          onPatch({ archiveLevel: next })
          onChangeArchiveLevel(next)
        }}
      />
      <small>Level {settings.archiveLevel}</small>
    </div>
  {/if}

  {#if showAfterExtractRow}
    <div class="form-label">After extract</div>
    <div class="form-control checkbox">
      <input
        type="checkbox"
        checked={settings.openDestAfterExtract}
        on:change={(e) => {
          const next = (e.currentTarget as HTMLInputElement).checked
          onPatch({ openDestAfterExtract: next })
          onToggleOpenDestAfterExtract(next)
        }}
      />
      <span>Open destination after extract</span>
    </div>
  {/if}

  {#if showRarNoteRow}
    <div class="form-label">Note</div>
    <div class="form-control">
      <p class="note">RAR compressed entries are currently unsupported (fail fast).</p>
    </div>
  {/if}
{/if}
