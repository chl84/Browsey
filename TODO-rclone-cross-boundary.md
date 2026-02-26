# TODO: rclone Cross-Boundary (Disk <-> Cloud)

Mål: støtte `local -> cloud` og `cloud -> local` for filer/mapper i Browsey, uten å bryte eksisterende `local <-> local` og `cloud <-> cloud`.

## V1 scope (låst)

- [ ] Støtt `copy` og `move` mellom lokal disk og `rclone://...`
- [ ] Støtt både filer og mapper
- [ ] `stop-on-first-error` for batch (ingen rollback i v1)
- [ ] Ingen undo for cross-boundary i v1
- [ ] Ingen mixed selection (lokal + cloud i samme paste/drag) i v1

## Backend foundation

- [x] Opprett egen cross-boundary transfer-modul (f.eks. `src/commands/transfer/`)
- [x] Definer tydelige request/response-typer for mixed preview + copy/move
- [x] Legg til Tauri commands for mixed conflict preview
- [x] Legg til Tauri commands for mixed `copy`
- [x] Legg til Tauri commands for mixed `move`
- [x] Gjenbruk eksisterende cloud path/parser/provider (`rclone`) der det er mulig

## Konfliktpreview (robust + effektiv)

- [x] Implementer `local -> cloud` preview med én cloud dest-listing (ikke `stat`-loop)
- [x] Implementer `cloud -> local` preview med én lokal dest-listing (ikke `stat`-loop)
- [ ] Behold rename-on-conflict (`-1`, `-2`, ...) med reservasjon i minnesett
- [x] Bruk provider-aware navnesammenligning for cloud mål (OneDrive case-insensitiv)
- [x] Returner payload kompatibel med eksisterende konfliktmodal

## Execute (copy/move)

- [x] Implementer `local -> cloud` file copy/move via `rclone` (filer; mapper senere)
- [x] Implementer `cloud -> local` file copy/move via `rclone` (filer; mapper senere)
- [ ] Verifiser og implementer mappe-copy/move semantikk eksplisitt (ikke anta)
- [ ] Koble `overwrite` / `rename` policy til preview-resultat (`prechecked=true` der mulig)
- [x] Behold provider-aware error mapping for mixed ops

## Frontend routing + UX

- [x] Utvid paste-route klassifisering (`local`, `cloud`, `local_to_cloud`, `cloud_to_local`, `unsupported`)
- [x] Koble mixed conflict preview til eksisterende konfliktmodal
- [x] Koble mixed execute til eksisterende paste-flyt (`handlePasteOrMove`)
- [x] Behold korrekt activity-label (`Copying…` / `Moving…`) for mixed ops
- [x] Behold cut-clipboard clear etter vellykket move (ingen halvtone-regresjon)
- [x] Bruk bakgrunns-refresh/soft-fail der refresh er treg (spesielt cloud-destinasjon)

## Drag-and-drop og shortcuts

- [ ] Aktiver `local -> cloud` drag-and-drop
- [ ] Aktiver `cloud -> local` drag-and-drop
- [ ] Behold blokkering for mixed selection i samme drag
- [ ] Verifiser `Ctrl+C` / `Ctrl+X` / `Ctrl+V` begge retninger

## Robusthet og ytelse

- [ ] Unngå unødvendige ekstra metadata-kall i mixed preview/execute
- [ ] Bruk eksisterende cloud cache/retry der det gir mening
- [ ] Ikke gjør hele operasjonen til feil hvis write lykkes men refresh feiler
- [ ] Logg nok timing/feilkontekst til å feilsøke quota/timeouts

## Tester

- [ ] Backend tester for `local -> cloud` copy/move (file + dir)
- [ ] Backend tester for `cloud -> local` copy/move (file + dir)
- [ ] Backend tester for mixed conflict preview (rename/overwrite)
- [x] Frontend tester for route-klassifisering og konfliktmodal (mixed)
- [x] Frontend tester for `Moving…`/`Copying…` labels i mixed ops
- [x] Frontend tester for cut-state reset etter mixed move

## Manuell validering (OneDrive + Google Drive)

- [ ] `local -> cloud` copy file
- [ ] `local -> cloud` move file
- [ ] `local -> cloud` copy/move folder
- [ ] `cloud -> local` copy file
- [ ] `cloud -> local` move file
- [ ] `cloud -> local` copy/move folder
- [ ] Konfliktflyt (`rename` / `overwrite`) begge retninger
- [ ] Drag-and-drop begge retninger
- [ ] Shortcuts (`Ctrl+C/Ctrl+X/Ctrl+V`) begge retninger
- [ ] Refresh/F5 fallback og brukerfeil ved timeout/rate-limit

## PR-rekkefølge (anbefalt)

- [ ] PR1: Mixed route + conflict preview (ingen execute)
- [ ] PR2: `local -> cloud` copy/move execute
- [ ] PR3: `cloud -> local` copy/move execute
- [ ] PR4: Drag-and-drop + UX wiring + refresh
- [ ] PR5: Tester + manuell validering + docs
