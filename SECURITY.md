# Security Policy

## Reporting a Vulnerability

If you believe you have found a security vulnerability in this repository, please use GitHub's private vulnerability reporting feature.

Do not open a public issue for suspected security problems.

Please include, when possible:

  - a clear description of the issue
  - the affected crate, module, or surface
  - the impact you believe it may have
  - reproduction steps or a minimal proof of concept
  - any relevant environment details

  ## Scope

This repository publishes the LAIC mechanism-layer crates and release artifacts, including:

  - `latrix-laic` (Rust package; library crate name: `laic`)
  - `laicc`

Reports are most helpful when they clearly distinguish:

  - mechanism-layer transport or protocol issues
  - contract/code generation issues
  - release artifact or packaging issues

  ## Response Expectations

We will review incoming reports and try to confirm whether the issue is valid and in scope.

If the report is accepted, we will coordinate remediation and disclosure through GitHub security advisories when appropriate.

  ## Out of Scope

The following are generally out of scope for this repository's security policy unless they directly affect published LAIC release artifacts:

  - runtime policy decisions
  - service discovery or routing layers
  - provider hosting logic
  - unrelated downstream integration code
