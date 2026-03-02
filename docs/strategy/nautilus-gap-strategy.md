# Browsey vs. Nautilus: Gap Analysis and Strategy

## Purpose

This document turns the high-level comparison into an execution strategy.
The goal is not to copy Nautilus feature-for-feature. The goal is to identify
the gaps that matter most if Browsey should be credible as a daily-driver file
manager at the quality level users associate with Nautilus.

## Current Assessment

Browsey is already beyond the scope of a small prototype:

- The backend is large, domain-split, and test-backed.
- The frontend has explicit feature boundaries, lint/type/test gates, and a
  working explorer shell.
- The product already covers advanced workflows that many file managers never
  reach: duplicate scanning, archive flows, cloud remotes, undo/redo, metadata,
  thumbnails, remappable shortcuts, and conflict-aware transfer flows.

Browsey is not yet at Nautilus maturity because the remaining gaps are mostly
in the hard parts of desktop software:

- behavioral consistency under edge cases
- desktop integration depth
- accessibility and localization
- platform breadth
- long-tail polish in large real-world workloads

## Strategic Position

Browsey should not try to out-Nautilus Nautilus on GNOME integration first.
That is a losing sequencing decision.

The better position is:

- Keep Browsey fast, explicit, and keyboard-friendly.
- Be stronger than Nautilus in power-user workflows.
- Close the maturity gap first in reliability, polish, and operational safety.
- Add deeper desktop integration only after the core interaction model is
  predictably solid.

## Where Browsey Is Already Strong

- Backend-heavy architecture keeps filesystem and search work out of the UI.
- Explorer domain boundaries are clearer than in many single-file desktop apps.
- Cloud support via `rclone` gives Browsey reach that Nautilus often handles
  only through the surrounding desktop stack.
- Typed backend errors, cancellation-aware long-running tasks, and explicit
  service/state boundaries are strong engineering choices.
- The project already has CI for frontend lint/type/test/e2e/build and backend
  fmt/clippy/tests.

These are not trivial strengths. They mean the project can absorb more product
complexity without immediately collapsing into unmaintainable code.

## Primary Gaps vs. Nautilus

### 1. Reliability and Edge-Case Hardening

Nautilus has years of real-world abuse behind its behavior. Browsey still needs
more proof under:

- very large directories
- unusual filenames and encodings
- permission failures and partial-failure recovery
- removable drives, flaky network mounts, and stale paths
- cross-boundary operations between local, network, trash, and cloud paths
- restart/crash recovery after interrupted operations

This is the highest-priority gap because users judge file managers first on
whether they trust them not to lose data or get confused.

### 2. UX Polish and Interaction Consistency

Browsey already has breadth, but mature desktop products win on consistency:

- focus behavior must always be predictable
- keyboard, context menu, drag/drop, and breadcrumb flows must agree
- progress, cancellation, retry, and failure messaging must feel coherent
- dense features must not leak implementation details into the user experience

The difference between "feature-rich" and "mature" is often this layer.

### 3. Desktop Integration

Nautilus benefits from deep GNOME ecosystem integration. Browsey is behind on:

- system-level share/open/default-app expectations
- file association and launcher polish
- portal, sandbox, and desktop service integration depth
- native trash/network semantics across environments
- tighter mount, removable-media, and shell-behavior integration

Browsey should improve integration, but only where it materially reduces user
friction or failure risk.

### 4. Accessibility and Internationalization

This is a major maturity gap.

- screen reader support is not yet a visible project strength
- keyboard-only navigation should be audited end-to-end, not inferred
- color, contrast, and motion behaviors need systematic validation
- localization/internationalization is not yet positioned as a first-class area

Nautilus-level maturity is not credible without this work.

### 5. Platform Parity

Browsey is explicitly Linux-first, while Windows is in maintenance mode and
macOS is unsupported. That is a reasonable scope decision, but it keeps the
product below "general-purpose mature file manager" status.

If cross-platform remains part of the product claim, parity gaps must narrow:

- file operation semantics
- permissions UX
- shell integration
- packaging and installer reliability
- platform-specific regression coverage

### 6. Cloud Maturity

Browsey is ambitious here, but cloud is still clearly v1:

- no cloud trash/recycle-bin integration
- no undo/redo for cloud operations
- partial feature coverage for thumbnails, archive flows, and open-with
- provider-specific rate-limit and consistency edge cases still need refinement

This does not block Browsey from being strong overall, but it blocks Browsey
from feeling equally mature across local and remote content.

### 7. Validation Depth

The project has meaningful automated checks, but Nautilus-level confidence would
require broader validation depth:

- more integration tests across command boundaries
- more end-to-end scenarios around destructive operations
- fixture-driven tests for malformed and hostile filesystem cases
- performance/regression baselines for large directories and search workloads
- better manual release checklists for critical workflows

## Complexity Assessment

Browsey is already complex enough that accidental regressions are a normal risk.
This is no longer "small app" complexity.

Current complexity drivers:

- mixed Rust/Tauri/Svelte stack
- local + cloud + network + trash + archive + metadata domains
- long-running tasks with cancellation and progress
- platform-specific behavior branches
- interactive explorer state with many input modes and view states

The implication is important: from this point onward, quality will improve less
from adding features and more from reducing behavioral ambiguity.

## What "Near Nautilus Level" Should Mean for Browsey

The target should be practical, not symbolic.

Browsey is near Nautilus level when:

- core file operations feel trustworthy under failure and interruption
- explorer interaction is stable and unsurprising across views and modes
- large-folder and search performance remains predictably good
- accessibility is good enough that it is not a known weak point
- Linux desktop integration feels intentional rather than partial
- cloud support is either clearly bounded or mature enough not to feel fragile

It does not require feature parity in every GNOME-specific capability.

## Recommended Strategy

### Phase 1: Trustworthiness First

Objective: make the product feel safe and predictable for daily use.

Focus areas:

- Expand destructive-operation integration tests and restart-recovery tests.
- Build a repeatable manual validation checklist for rename/move/copy/trash/
  delete/extract/conflict flows across local, network, and cloud paths.
- Add fault-injection coverage for permission denial, disappearing files,
  partially completed transfers, and backend task cancellation.
- Audit error messages so user-facing failures are actionable and consistent.
- Tighten logging around operation boundaries and recovery paths.

Definition of done:

- no known data-loss-class bugs in core local workflows
- release candidates pass a defined operations checklist
- failures degrade clearly instead of leaving uncertain UI state

### Phase 2: UX Consistency and Interaction Polish

Objective: reduce behavioral friction more than feature gaps.

Focus areas:

- Standardize focus restoration, modal defaults, and escape/enter behavior.
- Audit selection, drag/drop, breadcrumb, search, and context-menu consistency.
- Simplify or centralize explorer mode transitions where state remains fragile.
- Improve progress surfaces so queueing, partial completion, retry, and cancel
  semantics are obvious.
- Run deliberate polish passes on topbar, sidebar, properties, and search UX.

Definition of done:

- fewer interaction-specific regressions
- lower UI state ambiguity after cancellations and errors
- core workflows feel coherent without needing user adaptation

### Phase 3: Accessibility and Desktop Maturity

Objective: remove the most visible signs of "advanced but not yet mature".

Focus areas:

- Audit keyboard-only navigation for every major modal and explorer workflow.
- Add accessibility testing for labels, roles, focus order, and announcements.
- Formalize high-contrast and reduced-motion expectations.
- Review Linux desktop integration end-to-end: open-with, default apps, trash,
  mounts, removable media, and launcher/system behaviors.
- Decide whether localization is in scope; if yes, introduce an i18n framework
  before more user-facing strings accumulate.

Definition of done:

- accessibility no longer feels like a gap category
- Linux integration covers the common desktop expectations reliably

### Phase 4: Cloud and Platform Credibility

Objective: make non-local workflows feel intentional rather than provisional.

Focus areas:

- either deepen cloud support or narrow/clarify its supported contract
- add cloud-specific recovery, retry, and consistency tests
- close the most visible cloud parity gaps: trash semantics, open-with model,
  clearer capability restrictions, and better stale-cache handling
- improve Windows validation if cross-platform positioning remains important

Definition of done:

- users can predict what works on cloud paths and what does not
- platform support claims match actual regression coverage

## Suggested Priorities for the Next 6 Months

1. Harden core file operations and recovery paths.
2. Build a serious release checklist and keep it current.
3. Reduce explorer-state ambiguity through targeted UX cleanup.
4. Add accessibility auditing and keyboard-only verification.
5. Clarify the product stance on cloud maturity and Windows scope.

This ordering matters. It prevents the project from spending time on impressive
surface-area work while trust and predictability are still weaker than they
should be.

## Anti-Goals

Browsey should avoid these traps:

- chasing GNOME-specific parity before reliability gaps are closed
- adding major new feature domains before existing ones are behaviorally stable
- treating docs or screenshots as substitutes for release validation
- claiming broad platform support without commensurate test coverage
- letting cloud workflows silently inherit local-file assumptions

## Concrete Engineering Moves

These are the highest-leverage execution changes:

- Add a dedicated manual release checklist document for critical workflows.
- Add fixture-based regression suites for hostile filesystem cases.
- Add more end-to-end tests for destructive and recovery-sensitive paths.
- Create a small set of benchmark directories and search workloads.
- Track and triage bugs by trust-impact first, not by feature area.
- Prefer removing ambiguous behavior over adding configurability.
- Keep architecture boundaries strict as explorer complexity grows.

## Metrics to Track

Do not rely only on feature count. Track:

- regressions found after release in core file operations
- bugs involving ambiguous UI state after cancel/error/retry
- cloud-operation failure rate by category
- median and tail latency for load/search/thumbnail-heavy directories
- test coverage growth in destructive-operation and recovery scenarios
- accessibility issues found during audit passes

## Bottom Line

Browsey is already a serious file manager with meaningful technical depth.
The gap to Nautilus is now less about missing basic features and more about
maturity disciplines: consistency, recovery, accessibility, integration, and
proof under edge conditions.

If Browsey focuses the next stages on trustworthiness first, then interaction
polish, then accessibility and cloud/platform credibility, it can close the
most important maturity gap without losing the product's current strengths.
