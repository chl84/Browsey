# TODO: Secure and Practical Migration to `rclone rc`

Goal: improve cloud navigation/write performance by replacing hot-path per-command CLI spawns with a long-lived `rclone rcd` path, while keeping current behavior stable and secure.

Principles (keep it simple):
- Preserve current Tauri/frontend API contracts.
- Keep CLI as immediate fallback at all times.
- Ship in vertical slices (read path first, then write path).
- Add new abstractions only when needed.

## 0. Lock decisions (one-time)

- [x] Linux-first only for `rc` (Unix socket transport).
- [x] Keep CLI fallback always available in v1.
- [x] Scope phase-1 to read path only (`list remotes`, `list dir`, `stat`).
- [x] Defer write ops to phase-2+ after read path is stable.
- [x] Keep unsupported cloud features unchanged (undo, cloud trash, open-with, etc.).

## 1. Security baseline (must pass before enabling `rc`)

- [x] Use Unix socket only: `--rc-addr unix:///run/user/<uid>/browsey-rclone-<nonce>.sock`.
- [x] Ensure runtime dir/socket ownership + permissions are user-only (`0700` dir).
- [ ] Start `rclone rcd` with minimal surface:
  - [x] no `--rc-web-gui`
  - [x] no `--rc-files`
  - [x] no `--rc-serve`
  - [x] no metrics endpoint
- [x] Enforce backend method allowlist (no dynamic method names from UI).
- [ ] Keep payload/log redaction and avoid logging sensitive fields.

## 2. Lifecycle manager (minimal first)

- [x] Add `rclone rcd` manager module (`spawn`, `ready`, `shutdown`).
- [x] Lazy-start daemon on first cloud call.
- [x] Add bounded readiness timeout + health check.
- [x] Handle stale socket on startup (remove if orphaned).
- [x] Ensure clean shutdown on app exit (reuse current shutdown hooks).
- [x] Ensure single daemon instance per app process.

## 3. Integration strategy (smart simplification)

- [x] Do **not** start with a broad new trait hierarchy.
- [x] Add a narrow backend switch point in cloud provider code:
  - [x] `backend = rc if healthy, else cli`
- [x] Keep existing `CloudErrorCode` mapping unchanged at call sites.
- [x] Keep provider-specific mapping in provider layer (not in transport layer).

## 4. Phase-1 vertical slice: read path only

- [x] Implement `rc` calls for:
  - [x] remote discovery
  - [x] dir listing
  - [x] stat/existence
- [x] Keep existing caches/retry/concurrency policy wired the same.
- [ ] Ensure sorting/navigation does not add extra remote calls.
- [x] Add automatic fallback to CLI on `rc` connect/timeout/protocol failure.

Definition of Done (phase-1):
- [ ] Read path works end-to-end with unchanged frontend behavior.
- [ ] CLI fallback triggers automatically and safely.
- [ ] No security regression in socket exposure/logging.

## 5. Phase-2 vertical slice: write path

- [ ] Add `rc` mapping for `mkdir`.
- [ ] Add `rc` mapping for `copy`/`move`/`rename`.
- [ ] Add `rc` mapping for file/dir delete operations.
- [ ] Keep existing conflict/overwrite semantics identical.
- [ ] Keep mixed local<->cloud flow behavior unchanged.

Definition of Done (phase-2):
- [ ] Write path parity with current CLI behavior (incl. conflict handling).
- [ ] Fallback to CLI preserves operation correctness.

## 6. Reliability and performance hardening

- [ ] Add endpoint-class timeouts (`read`, `write`).
- [ ] Add bounded retries/backoff for transient errors.
- [ ] Keep bounded per-remote concurrency limits.
- [ ] Add cancellation for long-running write calls where supported.
- [ ] Add a short cooldown after rate-limit bursts (especially Google Drive).

## 7. Observability (minimal but useful)

- [ ] Structured logs for daemon lifecycle (start/ready/restart/shutdown).
- [x] Structured logs for method + latency + result (`rc` vs `cli` fallback).
- [ ] Extend `scripts/dev/rclone-perf-summary.sh` with `rc` buckets.
- [ ] Add one dev health check command (debug only) for daemon/socket status.

## 8. Testing (focus on parity + safety)

- [ ] Unit tests: lifecycle manager and fallback switching.
- [ ] Unit tests: allowlist enforcement and unsafe method rejection.
- [ ] Integration tests with fake `rc` server:
  - [ ] success path
  - [ ] timeout
  - [ ] malformed payload
  - [ ] unavailable socket
- [ ] Regression tests: conflict preview/rename-on-conflict parity.
- [ ] Frontend regression: no API contract change required.

## 9. Security verification checklist

- [ ] Verify no TCP listener is used in Linux mode.
- [ ] Verify socket path permissions on startup.
- [ ] Verify no credentials/secrets in logs on failures.
- [ ] Verify unsupported methods are rejected in backend.
- [ ] Verify stale socket recovery does not allow privilege crossing.

## 10. Rollout plan

- [x] Stage A: hidden read-path `rc` with CLI fallback default.
- [ ] Stage B: default read-path `rc` on Linux.
- [ ] Stage C: hidden write-path `rc` with fallback.
- [ ] Stage D: default write-path `rc` on Linux.
- [ ] Stage E: document final ops model + troubleshooting.
