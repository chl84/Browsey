# GLib 0.18.5 compatibility backport

Source: https://crates.io/crates/glib/0.18.5 (MIT; LICENSE and COPYRIGHT retained).
Archive SHA-256 is recorded in vendor/provenance.json.

Tauri's GTK3/WebKitGTK stack requires the 0.18 GLib/GIO ABI. Adding GLib 0.20
alongside it does not fix the older instance and creates incompatible types.
This local Cargo patch backports the exact fix from gtk-rs/gtk-rs-core#1343:
`VariantStrIter::impl_get` passes a mutable out pointer to the variadic C API.
No other upstream source code is changed. The package manifest allows only
the newer lifetime-style and unnecessary-parentheses lints in this old upstream
branch; application warnings remain denied.

Advisory: RUSTSEC-2024-0429. The application regression covers next, next_back,
nth, last and collect; it must also run with optimizations before release.
Remove this patch when the entire Tauri GTK/GIO stack supports a fixed upstream
ABI. The vendored source remains version 0.18.5, so version-only scanners may
still report the advisory; do not relabel it as upstream 0.20.
