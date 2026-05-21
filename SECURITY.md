# Security Policy

## Supported Scope

Security reports are in scope when they affect LAIC-owned release artifacts or
mechanism-layer behavior:

- `latrix-laic` / Rust crate `laic`;
- `laicc` library and CLI;
- `.laic` contract parsing, validation, and code generation;
- IPC and QUIC transport mechanism behavior;
- protocol framing, codecs, flow control, emergency delivery, gateway behavior,
  and the minimal trust-domain handshake;
- release packaging or source-package contents for official LAIC artifacts.

The following are not LAIC security scope by themselves:

- Runtime, Core, Secure, or downstream application integration behavior;
- discovery, routing, scheduling, provider hosting, workflow, marketplace, or
  multi-agent orchestration behavior;
- application authorization, IAM, ACL, business policy, or user account systems;
- public TCK, certification, or partner-ecosystem claims;
- benchmark-only performance comparisons.

If a report involves a downstream system and LAIC together, separate the
LAIC-owned mechanism issue from the downstream policy or integration issue as
clearly as possible.

## Supported Versions

Security review is focused on the current published MVP line and the current
`main` branch. Older versions may receive documentation guidance or a migration
recommendation, but patch handling depends on severity, practical exploitability,
and release-surface impact.

## Reporting A Vulnerability

Use GitHub private vulnerability reporting for this repository when available.
If that is not available, contact the maintainers through the repository owner
or project communication channel without posting exploit details publicly.
Do not open a public issue for suspected security problems.

Do not include secrets in a report. Redact tokens, credentials, private keys,
internal hostnames, customer data, and personal data. If a proof of concept is
needed, keep it minimal and focused on the LAIC-owned mechanism.

Please include:

- affected artifact or component;
- affected version, commit, or package source;
- operating system and Rust toolchain when relevant;
- minimal reproduction steps;
- expected result and actual result;
- whether the issue affects parsing/codegen, transport/framing, flow control,
  emergency delivery, handshake, packaging, or documentation;
- any known workaround.

## Handling Expectations

The maintainers will first classify whether the report is within LAIC scope. If
it is outside LAIC scope, the response may be a boundary clarification rather
than a code change.

For in-scope reports, the expected handling path is:

1. acknowledge receipt when maintainers see the report;
2. reproduce or narrow the issue;
3. classify severity and affected stable surface;
4. decide whether the fix is documentation-only, patch-level, or requires a new
   minor-version plan;
5. run the normal verification and release gates before any public release.

Passing CI or preparing a patch does not by itself publish a release, create a
tag, create a GitHub Release, or publish crates. Publication remains a separate
explicit approval step.

## Disclosure

Please give maintainers a reasonable opportunity to investigate and prepare a
fix before public disclosure. Coordinated disclosure should avoid publishing
working exploit details until an advisory, patch, or boundary decision is ready.
