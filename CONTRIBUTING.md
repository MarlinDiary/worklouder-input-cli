# Contributing to WorkLouderCTL

WorkLouderCTL is currently pre-alpha. Contributions should strengthen the
evidence, compatibility model, transaction safety, or command contract without
claiming support beyond the tested boundary.

## Good early contributions

- sanitized Input and device fixtures;
- schema and reference-validation tests;
- documented hardware observations with exact versions;
- read-only discovery and parsing;
- deterministic CLI/JSON contract proposals;
- documentation corrections and translations.

## Evidence requirements

For behavior changes, include:

1. the exact Input, firmware, device, transport, and OS boundary;
2. a baseline that demonstrates the previous behavior;
3. the smallest focused change;
4. automated fixture checks;
5. hardware readback when the claim involves a device write;
6. rollback evidence for mutations.

Avoid committing personal Smart Action payloads, commands, application paths,
device identifiers, or unsanitized backups.

## Pull requests

- keep each pull request narrow;
- explain the user-visible behavior and compatibility boundary;
- update the compatibility policy or roadmap when the claim changes;
- use clear commit messages and include the validation commands/results.

By contributing, you agree that your contribution is licensed under the MIT
License.
