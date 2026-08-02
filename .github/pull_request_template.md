## Summary

Describe the smallest user-visible change and the authority or tier it affects.

## Evidence boundary

- WorkLouderCTL version or commit:
- macOS, Codex, Input, device, firmware, and transport versions:
- Fixture, isolated provider, or live-provider boundary:

## Baseline and change

- Baseline command, literal result, and exit status:
- Modified command, literal result, and exit status:
- Changed branch, field, or contract:

## Verification and rollback

- [ ] Focused tests reproduce the baseline and verify the change.
- [ ] `cargo fmt --check`, `cargo test --locked`, and Clippy pass.
- [ ] Relevant compatibility, schema, generated-asset, packaging, and E2E gates pass.
- [ ] Mutations include fresh read, private backup, CAS, exact readback, and verified restore evidence.
- [ ] Unknown fields and unrelated configuration bytes are preserved.
- [ ] Documentation, compatibility matrix, changelog, and sanitized fixtures are updated where required.
- [ ] No credentials, identifiers, private commands, local paths, or unsanitized backups are included.
