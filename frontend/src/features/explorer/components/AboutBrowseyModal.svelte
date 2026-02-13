<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'
  import { loadAboutInfo, type AboutInfo } from '../services/about'

  export let open = false
  export let onClose: () => void = () => {}

  type AboutTab = 'version' | 'build' | 'license'

  const tabs: AboutTab[] = ['version', 'build', 'license']
  const tabLabels: Record<AboutTab, string> = {
    version: 'Version',
    build: 'Build',
    license: 'License',
  }

  const licenseText = (about: AboutInfo) =>
    `Browsey License\n\n${about.license.trim()}\n\nTHIRD_PARTY_NOTICES\n\n${about.thirdPartyNotices.trim()}`

  let activeTab: AboutTab = 'version'
  let info: AboutInfo | null = null
  let loading = false
  let loadError = ''
  let wasOpen = false

  const load = async () => {
    if (loading) return
    loading = true
    loadError = ''
    try {
      info = await loadAboutInfo()
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  $: {
    if (open && !wasOpen) {
      activeTab = 'version'
      if (!info) {
        void load()
      }
    }
    wasOpen = open
  }
</script>

{#if open}
  <ModalShell open={open} onClose={onClose} modalClass="about-modal" modalWidth="390px">
    <svelte:fragment slot="header">About Browsey</svelte:fragment>

    <div class="tabs">
      {#each tabs as tab}
        <button
          type="button"
          class:selected={activeTab === tab}
          on:click={() => {
            activeTab = tab
            if (!info && !loading) {
              void load()
            }
          }}
        >
          {tabLabels[tab]}
        </button>
      {/each}
    </div>

    {#if loading}
      <p class="muted">Loading…</p>
    {:else if loadError}
      <p class="pill error">Failed to load About data: {loadError}</p>
    {:else if info}
      {#if activeTab === 'version'}
        <div class="summary">
          <p class="name">{info.appName}</p>
          <p class="muted">Version {info.version}</p>
        </div>
        <pre class="doc">{info.changelog}</pre>
      {:else if activeTab === 'build'}
        <div class="rows">
          <div class="row"><span class="label">App</span><span class="value">{info.appName}</span></div>
          <div class="row"><span class="label">Version</span><span class="value">{info.version}</span></div>
          <div class="row"><span class="label">Profile</span><span class="value">{info.build.profile}</span></div>
          <div class="row"><span class="label">Target OS</span><span class="value">{info.build.targetOs}</span></div>
          <div class="row"><span class="label">Target Arch</span><span class="value">{info.build.targetArch}</span></div>
          <div class="row"><span class="label">Target Family</span><span class="value">{info.build.targetFamily}</span></div>
        </div>
      {:else}
        <section class="section">
          <h3>License + Third Party Notices</h3>
          <pre class="doc license-doc">{licenseText(info)}</pre>
        </section>
      {/if}
    {/if}

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  .summary {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-size: 18px;
    font-weight: 700;
    line-height: 1.2;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(96px, 130px) 1fr;
    gap: 10px;
    align-items: baseline;
  }

  .value {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
    word-break: break-word;
  }

  .doc {
    margin: 0;
    padding: 8px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    border-radius: 0;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 44vh;
    overflow: auto;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
    font-size: 12px;
    line-height: 1.35;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .license-doc {
    max-height: 52vh;
  }
</style>
