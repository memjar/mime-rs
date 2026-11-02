---
cairn: delta
change: the-installed-token
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: The CPU token is the application's

No mime-rs function SHALL detect CPU features or install a token. A leaf accessor and an RFC 2047 encoded word SHALL decode under `SimdCpu::current()`, the token the application installed or `SimdCpu::BASELINE`. The builder SHALL encode under `MimeBuildOptions::cpu` when it is some, and under `SimdCpu::current()` otherwise. The functions of `codec` SHALL take their token from the caller.

#### Scenario: An application that installs
- GIVEN a process whose `main` calls `SimdCpu::install_detected()`, then parses a message with a base64 leaf nested two multiparts deep
- WHEN the leaf is read with `decoded`, and the message is built back with the default `MimeBuildOptions`
- THEN both run the detected token's backend

### Requirement: The options

`MimeBuildOptions` SHALL state the line ending written (CRLF by default, LF for a local store), whether text may be `8bit` (8BITMIME), whether a leaf may be `binary` (BINARYMIME), whether fields may carry UTF-8 instead of encoded words (RFC 6532), whether `text/plain` with no `format` is written flowed (RFC 3676), whether a part with a non-ASCII `filename` also gets an encoded `name` on its Content-Type for old readers, encoded words in a parameter that RFC 2047 5 forbids and the validator reports, and the CPU token bodies are encoded under, the installed one when none is given.

## REMOVED Requirements
