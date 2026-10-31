---
cairn: delta
change: a-repo-for-mime-messages
---

## ADDED Requirements

### Requirement: No dependency on the syntax side

The decoded model, the spec layer, the builder and the validator SHALL compile with `--no-default-features` and pull in no crate beyond `core` and `alloc`. Everything byte-faithful SHALL sit under `tree`, gated on `parser`, and every dependency behind a feature.

#### Scenario: The bare core
- GIVEN the crate built with `--no-default-features`
- WHEN the dependency graph is inspected
- THEN it contains no crate beyond `core` and `alloc`

## MODIFIED Requirements

(none)

## REMOVED Requirements

(none)
