# Dependency maintenance

Dependency Security checks the committed Cargo lockfile and both npm lockfiles
on dependency changes and weekly. It includes development dependencies: build
and test tools are part of the supply chain, even when they are not shipped.
Rust vulnerabilities and npm advisories at moderate severity or higher fail CI.
Registry maintenance warnings remain visible; they are not silently ignored or
mislabelled as confirmed application vulnerabilities.

Local checks:

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo audit
npm --prefix frontend audit --audit-level=moderate
npm --prefix docs-site audit --audit-level=moderate
node scripts/maintenance/check-dependency-policy.mjs
node scripts/maintenance/check-vendor.mjs
node scripts/maintenance/check-pdfium.mjs
```

Keep security vulnerabilities, soundness warnings, and unmaintained-package
notices distinct. Check whether affected code is reachable on each supported
platform; a clean audit is not proof that an application is secure.

Upgrade in independently verified commits: security patches, frontend/tools,
the coordinated Tauri stack, native UnRAR, image/SVG dependencies, archive
libraries, then general backend libraries. Keep PDFium's ABI and bundled native
checksums pinned; see [PDFium maintenance](pdfium.md).

Run relevant regression suites after each step, and all Rust/frontend/browser
suites plus the production Tauri build before installation. Do not upgrade
GTK/GIO independently of the Tauri/WebKitGTK ABI. Native libraries must record
their actual compiled source revision, not merely their Rust wrapper version.

The GLib 0.18 ABI compatibility backport is a local Cargo patch. Registry-only
advisory checkers do not audit local source packages; `check-vendor.mjs` verifies
the entire patched source tree and CI exercises the affected iterator both
normally and with optimizations. See `vendor/glib/BROWSEY-PATCH.md`.

The optimized regression runs in the small `browsey-glib-regression` workspace
package, not the entire release-mode application. It imports the same test source
as Browsey and shares the root lockfile, GIO pin and vendored GLib patch. Ordinary
`cargo build`, `cargo test` and Tauri commands still select Browsey by default;
CI also lints and tests the harness with `--workspace`. Run its optimized test with:

```sh
cargo test --locked --release -p browsey-glib-regression --lib
```

Keep all mandatory PR checks, ordinary application tests and the native WebKit
test enabled. This optimization reduces compilation work, not test coverage.

UnRAR is also a local patch: stable native 7.23 (source package revision 7.2.7),
not just wrapper 0.5.8. Its unmodified source, patched bindings/build and native
licenses are hash-checked. Rust tests compare packed structure sizes/offsets
with a compiled C++ probe and exercise passwords, volumes and cancellation.
See `vendor/unrar-sys/BROWSEY-PATCH.md`.

## Automated proposals and protected dependencies

Dependabot proposes weekly compatible Cargo and npm updates in separate groups,
and GitHub Actions updates. It does not auto-merge. Major and 0.x-breaking library
upgrades require deliberate compatibility review. Tauri core/API/CLI, tauri-build,
GTK/GIO/GLib, PDFium and UnRAR are excluded from routine version proposals because
they need a coordinated source/ABI/license refresh. Watch their upstream advisories
manually; exclusions do not make them safe or keep local forks automatically current.
The dependency policy check catches mismatched Tauri JS/Rust pins and stale locks.

`upgrade-deps.sh` only updates Cargo.lock within existing Cargo.toml constraints;
it never runs `cargo upgrade` to widen explicit pins. Install the pinned audit
checker above before using the helper. A compatible update can still change
behaviour, add indirect dependencies or increase compiler requirements, so review
the lockfile diff and run the full suite before merging/installing.

## Verified upgrade baseline (2026-09-30)

- Tauri core/API/CLI 2.12.0 and tauri-build 2.7.0; GTK3/GIO 0.18 retained.
- TAR 0.4.46, ZIP 8.6.0, 7z 0.23.0; statically linked stable UnRAR 7.23.
- image 0.25.10, resvg 0.48.1 with maintained harfrust/skrifa font handling.
- rusqlite 0.40.2, sysinfo 0.39.6, Tokio 1.53.1 and Zstandard 0.14.0.
- Svelte 5.57.1+, Vite 7.3.6 and Vitest 4.1.11; compatible npm lockfile refreshes.
- PDFium stays pinned to the independently verified 8076/0.9.4 ABI profile.

Registry audits found no known vulnerabilities and both npm projects had zero
advisories. Two build-time maintenance warnings remain: `proc-macro-error` 1.0.4
through GTK3's GLib/GTK macros ([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html))
and `paste` 1.0.15 through EXR's SIMD support ([RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)).
Do not independently replace GTK3's macro stack, remove EXR support, or downgrade
the current EXR codec just to suppress these notices. Track upstream replacements
and re-evaluate when a compatible release is available. No advisory IDs are ignored.

Validation uses Rust 1.98.0 on Linux/Omarchy; sysinfo requires Rust 1.95 or later.
The final run passed Clippy with warnings denied, 516 Rust tests (three manual
integration tests excluded from the ordinary run), 261 frontend unit tests and
48 browser tests. The native WebKit export/window-teardown test was additionally
run on an isolated X11 screen, and the GLib iterator test passed against the
optimized release libraries. The production Tauri build embeds the frontend;
the shipped binary is not a plain Cargo development-server build. Cargo audit
and an OSV cross-check of all 591 registry packages reported only the two
maintenance notices above; local fork integrity is checked separately.

Windows runtime behaviour and clean Fedora/Ubuntu package installation still need
their platform-specific checks; Linux tests do not imply those have been validated.
