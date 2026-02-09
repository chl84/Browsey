<script lang="ts">
  type Section = {
    id: string
    title: string
    text: string
    bullets: string[]
  }

  const sections: Section[] = [
    {
      id: 'intro',
      title: 'What This Is',
      text: 'This site is the dedicated documentation project for Browsey, kept in the same repository but built and deployed independently from the desktop app.',
      bullets: [
        'Separate build pipeline from the main app',
        'Static HTML output using Vite + Svelte',
        'Ready for versioned docs pages',
      ],
    },
    {
      id: 'local-dev',
      title: 'Local Development',
      text: 'Run docs independently from the Tauri app so writing docs does not interfere with app development ports and tooling.',
      bullets: [
        'Install dependencies: npm --prefix docs install',
        'Start local server: npm --prefix docs run dev',
        'Build static output: npm --prefix docs run build',
      ],
    },
    {
      id: 'next-steps',
      title: 'Recommended Next Steps',
      text: 'If you continue with this setup, the next step is content structure: navigation, command references, release notes by version, and troubleshooting sections.',
      bullets: [
        'Add docs pages under src/routes-like sections',
        'Introduce version selector (latest + tagged versions)',
        'Publish docs/dist with your preferred static host',
      ],
    },
  ]
</script>

<main class="shell">
  <header class="top">
    <p class="eyebrow">Browsey Documentation</p>
    <h1>Dedicated Svelte Docs Workspace</h1>
    <p class="lede">
      The docs project now lives in <code>docs/</code> with its own package and build output.
    </p>
  </header>

  <div class="layout">
    <aside class="nav">
      <h2>Sections</h2>
      <nav aria-label="Section links">
        {#each sections as section}
          <a href={`#${section.id}`}>{section.title}</a>
        {/each}
      </nav>
    </aside>

    <section class="content">
      {#each sections as section}
        <article id={section.id} class="card">
          <h2>{section.title}</h2>
          <p>{section.text}</p>
          <ul>
            {#each section.bullets as bullet}
              <li>{bullet}</li>
            {/each}
          </ul>
        </article>
      {/each}
    </section>
  </div>
</main>
