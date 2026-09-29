# Dependency maintenance

Dependency Security checks the committed Cargo lockfile and both npm lockfiles
on dependency changes and weekly. It includes development dependencies: build
and test tools are part of the supply chain, even when they are not shipped.
During the staged upgrade the checks report the baseline; the final stage makes
security checks blocking after fixes and explicitly documented exceptions.

Local checks:

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo audit
npm --prefix frontend audit --audit-level=moderate
npm --prefix docs-site audit --audit-level=moderate
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
