## Summary

- Describe the change.

## Verification

- Mark the checks that apply and add any focused command used for this change.
- Docs-only changes may use `git diff --check` plus the relevant docs/site
  validator. UI-copy changes should include the focused source/layout test.
- Rust/product changes still require the full applicable quality and security
  set below. Report skipped checks and the reason honestly.

- [ ] Focused test or validator: `<command>`
- [ ] `npm test`
- [ ] `cargo test --offline`
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- [ ] `npm audit --audit-level=high`
- [ ] `cargo audit`

## Risk

- Describe the main risk and rollback path.

## Documentation

- [ ] Updated docs, or verified this change does not need docs.
