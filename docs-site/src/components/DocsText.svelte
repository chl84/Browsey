<script lang="ts">
  import { inlineParts } from '../lib/docs'
  export let text: string
</script>

{#each inlineParts(text) as part, index (index)}
  {#if part.code}<code>{part.text}</code>{:else}{part.text}{/if}
{/each}
