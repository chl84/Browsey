<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  export let open = false
  export let path = ''
  export let error = ''
  export let onSubmit: (password: string) => void
  export let onCancel: () => void
  let password = ''
  let showPassword = false
  $: if (!open) { password = ''; showPassword = false }
  const submit = () => {
    const value = password
    password = ''
    onSubmit(value)
  }
</script>

{#if open}
  <ModalShell {open} onClose={onCancel} initialFocusSelector="#extract-password">
    <svelte:fragment slot="header">Archive password</svelte:fragment>
    <p class="archive-path">{path}</p>
    {#if error}<div class="pill error" role="alert">{error}</div>{/if}
    <form id="archive-password-form" on:submit|preventDefault={submit}>
      <label class="field">
        <span>Password</span>
        <input id="extract-password" type={showPassword ? 'text' : 'password'} bind:value={password}
          autocomplete="off" spellcheck={false} autocapitalize="none" />
      </label>
      <label><input type="checkbox" bind:checked={showPassword} /> Show password</label>
    </form>
    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel extraction</button>
      <button type="submit" form="archive-password-form">Extract</button>
    </div>
  </ModalShell>
{/if}

<style>
  .archive-path { overflow-wrap: anywhere; user-select: text; }
</style>
