# TODO: rclone Cloud Performance (OneDrive først)

## Fase 0: Baseline og måling

- [x] Legg inn enkel timing/logging for cloud operasjoner (`conflict preview`, `write`, `refresh`)
- [ ] Definer og kjør baseline-test i OneDrive testmappe (navigering, copy, rename, delete)
- [ ] Noter før/etter-tider for hver optimalisering

## Fase 1: Refresh-ytelse (høy UX-effekt)

- [x] Cloud paste: bakgrunns-refresh i stedet for å blokkere hele operasjonen
- [x] Innfør refresh-koordinator (single-flight) per cloud-mappe
- [x] Debounce/coalesce flere refresh-requests tett på hverandre
- [x] Bruk samme bakgrunns-refresh-mønster for cloud `rename`
- [x] Bruk samme bakgrunns-refresh-mønster for cloud `mkdir`
- [x] Bruk samme bakgrunns-refresh-mønster for cloud `delete`
- [x] Bruk samme bakgrunns-refresh-mønster for cloud `move`

## Fase 2: Færre rclone-kall i paste/conflict

- [x] Bytt `preview_cloud_conflicts` til én listing av dest-mappe (ikke `stat` per source)
- [x] Bygg konfliktsett i minne fra dest-listing
- [x] Fjern `statCloudEntry`-loop ved rename-on-conflict i frontend
- [x] Unngå dobbel overwrite-precheck når konfliktpreview allerede er gjort
- [ ] Verifiser at cloud paste gjør færre `lsjson/--stat`-kall i logg

## Fase 3: Caching for navigering og Network-view

- [x] Cache cloud remote discovery (`listremotes + config dump`) med TTL
- [x] Invalider remote-cache ved eksplisitt Network-refresh
- [x] Cache cloud directory listing (`rclone://...`) med kort TTL
- [x] Invalider listing-cache ved cloud write-ops (copy/move/rename/mkdir/delete)
- [x] Reduser dobbel cloud-listing i `list_facets` (bruk cache eller eksisterende entries)

## Fase 4: Robusthet under treghet / throttling

- [x] Bounded concurrency per remote (unngå for mange samtidige rclone-kall)
- [x] Kort retry/backoff for metadata/listing (`lsjson`) ved transient feil
- [x] Forbedre cloud UX-meldinger ved treg refresh (ikke rapporter write som feil)
- [ ] Verifiser oppførsel ved flere raske operasjoner etter hverandre

## Fase 5: Tester og verifisering

- [x] Backend-tester for batch konfliktpreview
- [x] Backend-tester for cache TTL + invalidering
- [x] Frontend-tester for refresh-koordinator / coalescing
- [x] Frontend-tester for bakgrunns-refresh på rename/mkdir/delete
- [ ] Manuell OneDrive ytelsestest (samme sjekkliste som baseline)
- [x] Oppdater `TODO-rclone.md` med perf-status når første pakke er ferdig

## Anbefalt PR-rekkefølge

- [x] PR1: Refresh-koordinator + bakgrunns-refresh for cloud write-ops
- [x] PR2: Batch konfliktpreview + fjern `stat`-loop i rename-on-conflict
- [x] PR3: Remote discovery cache (Network-view)
- [x] PR4: Cloud listing cache + invalidering
- [x] PR5: Cloud facets uten dobbel listing
- [x] PR6: Bounded concurrency + metadata retry/backoff
