# TODO: Backend Hardening to 5/6 (Commands-first)

## Kort oppsummering

Vi lager en aktiv, kjørbar TODO i `docs/todo/` som er den operative planen for
å nå hardening 5/6 (commands-first, 2 sprinter, gradvis blokkerende CI,
Semgrep inkludert). Planen skal være beslutningskomplett og kunne brukes
direkte i PR-arbeid.

## 1) Header og mål

Created: 2026-03-03  
Target: 5/6 hardening confidence  
Scope: `src/commands/**`

Goal: forhindre typed-error-regresjoner med lagdelt kontroll (guard + semantisk lint + målrettet opprydding).

## 2) Baseline (låses i dokumentet)

- [ ] `from_external_message(...to_string())` i `src/commands/**`: **11** treff
- [ ] `map_err(...to_string())` i `cloud/transfer/network/permissions`: **5** treff (for tiden allowlistet/akseptert)
- [ ] `impl From<...> for String` i `src/commands/**`: **0** (utenfor scope finnes `metadata`/`watcher`)
- [x] Rust quality workflow: aktiv (`fmt`, `clippy`, guard, tests)

## 3) Scope / Out of scope

In scope:
- `src/commands/**`
- `scripts/maintenance/check-backend-error-hardening-guard.sh`
- `.github/workflows/rust-quality.yml`
- `scripts/maintenance/test-backend.sh`

Out of scope (for denne TODO-en):
- `src/metadata/**`, `src/watcher.rs`, `src/statusbar/**`
- frontend feature-endringer

## 4) Sprint 1 TODO (regelverk + lav-risiko opprydding)

- [x] Legg til Semgrep-regelsett i `.semgrep/typed-errors.yml`
- [x] Legg Semgrep i CI som **advisory** først
- [x] Rydd `from_external_message(...to_string())` i:
- [x] `src/commands/keymap.rs`
- [x] `src/commands/listing/mod.rs`
- [x] `src/commands/search/worker.rs`
- [x] `src/commands/entry_metadata/mod.rs`
- [x] `src/commands/duplicates/mod.rs`
- [x] `src/commands/compress/mod.rs`
- [x] Legg/oppdater tester som låser `code_str()` for berørte mappinger
- [x] Oppdater guard-policy: ingen nye allowlist-unntak uten begrunnelse + ref

## 5) Sprint 2 TODO (gradvis blocking)

- [ ] Gjør Semgrep **blocking** for `transfer/cloud/network/permissions`
- [ ] Flytt flere moduler til blocking når de er ryddet
- [ ] Fjern resterende funn i commands-first-scope eller dokumenter nødvendige boundaries
- [ ] Dokumenter unntaks-policy kort i docs (hvorfor/krav/test)

## 6) Kvalitetsporter (må være grønne)

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `bash scripts/maintenance/check-backend-error-hardening-guard.sh`
- [x] `cargo test --all-targets --all-features`
- [ ] `semgrep --config .semgrep/typed-errors.yml src/commands` (lokalt blokkert: `semgrep` ikke tilgjengelig i dette miljøet)

## 7) Akseptansekriterier for 5/6

- [ ] Ingen nye typed-error regressjoner i `src/commands/**`
- [ ] 0 blocking-funn i Semgrep-scope
- [ ] 0 blocking-funn i guard
- [ ] API-koder stabile (ingen utilsiktede `code`-endringer)

## 8) Exit + arkivering

- [ ] Når alle punkter er grønne: flytt filen til `docs/todo-archive/`
- [ ] Legg kort "completion note" i arkivfilen med dato og resultat

## Testscenarier som eksplisitt skal dekkes i TODO-arbeidet

1. Mapping-bevaring: upstream typed error -> samme forventede `code_str()`.
2. Ingen ny string-roundtrip i moduler som er satt til blocking.
3. CI-regresjon: PR skal feile ved nye blocking-funn.
4. Guard-regresjon: nye unntak uten begrunnelse skal avvises i review.

## Antakelser og valgte defaults

1. Scope er `commands-first`.
2. Leveranse over to sprinter.
3. Enforcement er gradvis blokkerende, ikke big-bang.
4. Semgrep tas inn i CI (advisory -> blocking).
5. Frontend/API-kontrakter holdes stabile.
