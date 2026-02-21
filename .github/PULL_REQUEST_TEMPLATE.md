## Summary

Describe what changed and why.

## Quality Checklist

- [ ] Docs updated (if behavior, architecture, or workflows changed)
- [ ] `CHANGELOG.md` updated (if user-visible behavior changed)
- [ ] Quality gates are green for affected areas:
  - [ ] Frontend: `npm --prefix frontend run lint`
  - [ ] Frontend: `npm --prefix frontend run check`
  - [ ] Frontend: `npm --prefix frontend run test`
  - [ ] Frontend: `npm --prefix frontend run build`
  - [ ] Rust: `cargo fmt --all -- --check`
  - [ ] Rust: `cargo clippy --all-targets --all-features -- -D warnings`
  - [ ] Rust: `cargo test --all-targets --all-features`
- [ ] Tests added/updated for changed behavior (or rationale provided below)

## Notes

List manual validation performed and any known follow-up work.
