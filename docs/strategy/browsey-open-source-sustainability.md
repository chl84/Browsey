# Browsey: Open Source Sustainability Assessment

## Purpose

This document evaluates Browsey from an open-source sustainability perspective:

- can the project remain maintainable over time
- is the architecture suitable for outside contribution
- what threatens long-term contributor and maintainer health

## High-Level Assessment

Browsey has a stronger sustainability foundation than many independent desktop
projects.

Reasons:

- the repository has visible structure
- domains are separated in a reasonably legible way
- documentation exists beyond README-level marketing
- CI gates are present for both frontend and backend
- the project appears to refactor as complexity grows

That said, Browsey has already crossed into a complexity band where
sustainability will depend much more on process than on code taste.

## Sustainability Strengths

### 1. Architecture Is Contributor-Friendly by Default

The top-level split between Rust backend, Svelte frontend, docs, scripts, and
resources is easy to reason about.

Within the app:

- backend commands are separated by domain
- shared backend subsystems are isolated
- frontend features are split into explicit ownership areas
- service/state boundaries are visible instead of implicit

This matters because new contributors can usually form a mental model without
reading the whole codebase first.

### 2. The Project Has Process Signals, Not Just Code Signals

Browsey already has some of the mechanisms that prevent long-term decay:

- frontend lint/type/test/e2e/build checks
- backend fmt/clippy/test checks
- architecture notes
- docs for behavior, not just setup
- release-note style change tracking

That gives the project a better chance of staying coherent as it grows.

### 3. The Stack Is Reasonable for Long-Term Maintenance

Rust plus Tauri plus Svelte is not the simplest stack, but it is defendable:

- Rust is a strong fit for filesystem-heavy and safety-sensitive logic
- Tauri keeps packaging and bridge boundaries relatively explicit
- Svelte keeps UI code smaller than many heavier frontend stacks

The stack is not inherently a sustainability liability.
The real risk comes from cross-boundary product complexity, not from the core
technology choices themselves.

## Sustainability Risks

### 1. Bus Factor Risk

This appears to be a highly concentrated project.
That is normal for independent software, but it creates obvious fragility:

- architectural knowledge may be too centralized
- release validation may depend too much on one person's memory
- priorities may shift faster than documentation and tests can keep up

This is the single biggest sustainability risk.

### 2. Feature Surface Risk

Browsey is no longer a narrowly scoped explorer.
It already includes:

- local file management
- trash and undo
- thumbnails and metadata
- archive pipelines
- search and duplicate scan
- network discovery
- cloud integration
- platform-specific behavior

Each of these areas can generate bugs, support questions, and design debt.

### 3. Desktop QA Burden

Desktop software is expensive to sustain because problems are often:

- environment-specific
- hardware-specific
- distro-specific
- shell-integration-specific
- hard to reproduce in CI

This burden grows faster than the codebase alone suggests.

### 4. Cloud and Cross-Platform Scope Risk

Cloud and cross-platform support are both strategically attractive, but they are
also maintenance multipliers.

If they are not tightly scoped, they can drain effort from the core local UX
that the product depends on.

## What Makes Browsey Maintainable Today

Browsey is maintainable today because:

- the codebase shows intentional decomposition
- module ownership is increasingly explicit
- quality gates exist
- the repo contains technical docs, not just promotional text
- there is evidence of ongoing cleanup and standardization

That is enough to support continued growth.
It is not enough to guarantee sustainability by itself.

## What Would Improve Sustainability Most

### 1. Make the Operating Model More Explicit

The project would benefit from a clearer maintainer model:

- what counts as stable vs. experimental
- what platforms are actively supported vs. best-effort
- what cloud capabilities are in contract vs. provisional
- how releases are validated before shipping

This reduces ambiguity for both users and contributors.

### 2. Convert Knowledge into Repeatable Assets

The next sustainability step is not mostly new code.
It is converting implicit maintainer knowledge into artifacts:

- release checklists
- regression scenario lists
- triage labels
- contribution boundaries
- platform support statements
- issue templates for high-risk bug categories

### 3. Protect the Core with Stronger Scope Discipline

The project should defend its core proposition aggressively:

- local file operations must stay strongest
- cloud should not quietly redefine every subsystem
- platform claims should match regression coverage
- new features should compete against hardening work for priority

This is how independent projects avoid slow erosion.

### 4. Improve Onboarding for Outside Contributors

The architecture is already reasonably well suited for contributions, but
onboarding could be improved further with:

- a contributor guide mapped to repo domains
- "good first issue" curation by subsystem
- a short testing matrix for common workflows
- explicit notes on where product decisions are still fluid

## Sustainability Outlook

### Best-Case Path

Browsey becomes a durable Linux-first open-source desktop application with:

- strong local workflow reliability
- a clear and bounded cloud story
- a disciplined release process
- enough documentation and test coverage that contribution is practical

### Failure Path

Browsey risks becoming difficult to sustain if:

- feature scope grows faster than regression protection
- cloud and platform breadth outpace validation depth
- too much behavior knowledge remains informal
- maintainer attention gets spread across too many strategic directions

## Recommended Sustainability Priorities

1. Write and maintain a release validation checklist for critical workflows.
2. Define support tiers for Linux, Windows, and cloud capabilities.
3. Increase regression coverage around destructive and recovery-sensitive flows.
4. Document subsystem boundaries and contributor entry points more explicitly.
5. Treat bug classes involving trust, ambiguity, or data safety as top priority.

## Bottom Line

Browsey looks sustainable in principle because the architecture is structured,
the stack is defensible, and the project already shows signs of technical
discipline.

The risk is not that the foundation is weak.
The risk is that the product is now complex enough that sustainability must be
managed deliberately.

If the project keeps converting complexity into structure, tests, docs, and
clear scope decisions, it has a realistic path to long-term maintainability as
an open-source desktop application.
