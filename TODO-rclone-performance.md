# TODO: rclone Cloud Performance (OneDrive først)

## Fase 0: Baseline og måling

- [ ] Legg inn enkel timing/logging for cloud operasjoner (`conflict preview`, `write`, `refresh`)
- [ ] Definer og kjør baseline-test i OneDrive testmappe (navigering, copy, rename, delete)
- [ ] Noter før/etter-tider for hver optimalisering

## Fase 1: Refresh-ytelse (høy UX-effekt)

- [x] Cloud paste: bakgrunns-refresh i stedet for å blokkere hele operasjonen
- [x] Innfør refresh-koordinator (single-flight) per cloud-mappe
- [x] Debounce/coalesce flere refresh-requests tett på hverandre
- [ ] Bruk samme bakgrunns-refresh-mønster for cloud `rename`
- [ ] Bruk samme bakgrunns-refresh-mønster for cloud `mkdir`
- [ ] Bruk samme bakgrunns-refresh-mønster for cloud `delete`
- [ ] Bruk samme bakgrunns-refresh-mønster for cloud `move`

## Fase 2: Færre rclone-kall i paste/conflict

- [ ] Bytt `preview_cloud_conflicts` til én listing av dest-mappe (ikke `stat` per source)
- [ ] Bygg konfliktsett i minne fra dest-listing
- [ ] Fjern `statCloudEntry`-loop ved rename-on-conflict i frontend
- [ ] Unngå dobbel overwrite-precheck når konfliktpreview allerede er gjort
- [ ] Verifiser at cloud paste gjør færre `lsjson/--stat`-kall i logg

## Fase 3: Caching for navigering og Network-view

- [ ] Cache cloud remote discovery (`listremotes + config dump`) med TTL
- [ ] Invalider remote-cache ved eksplisitt Network-refresh
- [ ] Cache cloud directory listing (`rclone://...`) med kort TTL
- [ ] Invalider listing-cache ved cloud write-ops (copy/move/rename/mkdir/delete)
- [ ] Reduser dobbel cloud-listing i `list_facets` (bruk cache eller eksisterende entries)

## Fase 4: Robusthet under treghet / throttling

- [ ] Bounded concurrency per remote (unngå for mange samtidige rclone-kall)
- [ ] Kort retry/backoff for metadata/listing (`lsjson`) ved transient feil
- [ ] Forbedre cloud UX-meldinger ved treg refresh (ikke rapporter write som feil)
- [ ] Verifiser oppførsel ved flere raske operasjoner etter hverandre

## Fase 5: Tester og verifisering

- [ ] Backend-tester for batch konfliktpreview
- [ ] Backend-tester for cache TTL + invalidering
- [ ] Frontend-tester for refresh-koordinator / coalescing
- [ ] Frontend-tester for bakgrunns-refresh på rename/mkdir/delete
- [ ] Manuell OneDrive ytelsestest (samme sjekkliste som baseline)
- [ ] Oppdater `TODO-rclone.md` med perf-status når første pakke er ferdig

## Anbefalt PR-rekkefølge

- [ ] PR1: Refresh-koordinator + bakgrunns-refresh for cloud write-ops
- [ ] PR2: Batch konfliktpreview + fjern `stat`-loop i rename-on-conflict
- [ ] PR3: Remote discovery cache (Network-view)
- [ ] PR4: Cloud listing cache + invalidering
- [ ] PR5: Cloud facets uten dobbel listing
- [ ] PR6: Bounded concurrency + metadata retry/backoff
