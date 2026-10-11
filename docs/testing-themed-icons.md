# Themed icons

Browsey's 53 file, folder, sidebar and permission SVGs live in
`resources/icons/scalable/`. This is the only rendered asset set; older frontend
copies are removed. Each asset has literal default colors from a common neutral palette,
with `data-icon-fill` / `data-icon-stroke` annotations identifying semantic roles.
Native SVG thumbnail renderers can display these files without CSS-variable
support. The sprite compiler converts the annotations to theme variables when
bundling the assets. `BrowseyIconSprite` mounts their definitions
once, and `BrowseyIcon` references them with `<use>`. Path-shaped icon keys and
backend icon IDs remain stable; they identify symbols rather than fetched files.
Root presentation attributes are preserved and internal gradient IDs are scoped
to each symbol. Only bundled assets enter the sprite.

All 21 document icons share one paper outline, folded corner and neutral base.
Folder variants share an angular body and tab, including cloud/network folders.
Structural contours use 2 units, standard line details 3 units, and standalone clock/permission
outlines 8 units on a 128-unit canvas. Semantic shapes may remain curved, such as
discs, database cylinders and eyes. Meaningful details use opaque palette colors;
shading uses explicit palette roles rather than inconsistent alpha values.
The PDF label is vector geometry, avoiding platform font differences.

Shared file/folder symbols are defined once in `scripts/icons/glyphs.mjs` and
rendered into self-contained SVGs by `scripts/icons/generate.mjs`, using the
generic file/folder bodies. Font Aa, code, document, music, picture, video, book,
gear and box pairs have identical geometry, placement and stroke widths. Only
semantic paint roles change with the surrounding body. Configuration and
executable files use the same gear as configuration folders.

After changing a shared glyph or base body, run
`npm --prefix frontend run icons:generate`. Generated SVGs are checked in so
native renderers and the browser use the same assets; `npm --prefix frontend run
lint:icons` (also part of lint) rejects stale output without modifying files.

Folder matching uses the full name, case-insensitively, in the shared local/cloud
resolver. New categories cover temp/tmp, backup/backups, archive/archives,
projects/development/dev/src/source, config/.config/settings, cache/.cache,
fonts/.fonts, books/ebooks, games and screenshots. Shared/share reuse public's
icon. Norwegian aliases cover Nedlastinger, Dokumenter, Bilder, Musikk, Videoer,
Skrivebord and Maler. These rules do not change file or symlink icon selection.

`theme/iconPalette.ts` derives a semantic palette from the active theme. It uses
the Omarchy colors when Browsey follows the system theme, or built-in colors for
light/dark mode. Changes go through the existing theme controller, with no
per-entry color calculation during scrolling.

The palette targets at least 3.2:1 contrast for primary silhouettes and 4.5:1 for
document ink, permission symbols and meaningful inset glyphs. High-contrast mode
raises the silhouette target to 4.5:1. Light documents use a contrasting outline;
dark documents use a light body. Decorative folds, panels and shading retain
subtler tonal differences. Folder inset surfaces also keep 4.5:1 contrast against
their glyph strokes; shared glyphs use these insets for readable inner details.
Color adjustment preserves a theme color when it
already meets the target, otherwise blending toward black or white only as far
as necessary. For incompatible background luminances, the fallback maximizes
the worst contrast; this is not a blanket accessibility guarantee for arbitrary
custom palettes.

Normal palettes account for the main, sidebar, hover, raised and composited drop
backgrounds. Selected rows/cards use a separate palette calculated against their
composited selection fill. A drop target uses the normal palette even when also
selected, matching the surface shown by the existing row/card styles. Hidden and
cut entries retain their existing deliberate opacity treatment.

Real photo, video, PDF and PSD thumbnails remain `<img>` elements with their
original colors. A failed thumbnail falls back to the themed symbol through the
existing loader's invalidation flow.

## Automated checks

```sh
npm --prefix frontend test -- src/features/explorer/theme/iconPalette.test.ts src/shared/icons/BrowseyIcon.test.ts
npm --prefix frontend run test:e2e -- theme-icons.e2e.ts model-icon.e2e.ts thumbnails.e2e.ts zoom.e2e.ts mtp-grid-stability.e2e.ts
cargo test --locked icons::
```

Palette checks cover light/dark, Nord, Ristretto, translucent selections, bright
selections on dark themes and high contrast. Sprite checks validate all assets,
unique definitions, gradient references and accessible permission labels.
Native SVG rendering checks cover all 53 assets and verify that all
document previews share the same neutral paper color and folded corner.
Pair checks compare all shared symbol geometries, including stroke widths and
transforms. Resolver checks cover every alias in upper/lower case, local/cloud
agreement, file/symlink behavior and names that must remain generic folders.
Browser checks exercise theme switching, selection/drop overrides, existing
list/grid sizes, thumbnail fallback and shared definitions. Existing thumbnail,
zoom and MTP scrolling regressions remain part of this verification.

## Visual acceptance

1. Inspect the sidebar and a folder with varied file types in list and grid.
   Include specialized folders, permissions, network, storage and the trash icon.
2. Switch light, dark and system themes while the folder stays open. Confirm that
   folders follow the accent, documents share the theme's neutral tint, and
   category colors distinguish types without clashing with the surrounding UI.
3. Select items, hover them and drag over valid/invalid folder targets. Check the
   exterior silhouette and the symbol inside each icon on the displayed surface.
4. Repeat with high contrast and small list/sidebar icons. Check that thin lines
   remain recognizable; numerical color contrast alone does not establish this.
5. Confirm that actual thumbnails keep their image colors, including after theme
   switching. A missing/corrupt thumbnail should become a themed file icon.
6. Scroll rapidly through a large folder in native WebKit, checking for missing
   icons, flashing or crashes. Exercise the user's actual GPU/Wayland session
   separately from a virtual-display test.

The original 43-icon set was also visually inspected in the collaborative
browser in Ristretto, light, dark and dark high-contrast themes. Each overview
renders all 43 actual sprite symbols at 64 and 18 pixels on normal and selected
surfaces, using their computed theme colors. Native rendering of the literal SVG
files is covered separately by the Rust icon tests.

The current shared file/folder pairs and ten new folder icons were inspected at
64 and 18 pixels in Ristretto, light, dark and dark high-contrast modes, on normal
and selected surfaces. The paired overviews render actual sprite definitions
with their computed colors. Light-mode inset contrast was checked after the
palette adjustment, alongside the automated contrast checks.

The initial implementation was inspected in WebKitGTK 2.52.6 on an isolated Xvfb
display: all 43 symbols on normal/selected surfaces in five theme combinations,
plus alternating scroll positions through 1,000 mock entries. Hardware policy
was `ALWAYS` with the DMABUF renderer disabled; Xvfb used software rendering.
This verifies native SVG/style rendering, not physical GPU behavior or the
previous Wayland scrolling issue.
