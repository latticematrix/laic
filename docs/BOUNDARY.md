# LAIC Boundary

LAIC is a mechanism-layer protocol project for high-throughput communication
between AI system components. It defines transport, contract compilation,
flow control, transport security, and emergency delivery mechanisms.

LAIC does not define runtime policy, discovery, routing, provider hosting,
business logic, resource placement, model selection, or client convenience
SDK behavior.

## Core Boundary

LAIC is responsible for:

- local IPC transport;
- remote QUIC transport;
- typed skill contracts and code generation;
- data-plane serialization boundaries;
- control-plane message boundaries;
- credit-based flow control;
- emergency delivery channels;
- transport security primitives;
- the minimal trust-domain handshake currently described by the stable surface.

LAIC is not responsible for:

- deciding where work should run;
- discovering providers;
- routing between providers;
- scheduling workloads;
- owning provider lifecycle;
- retry, backoff, or reconnect policy;
- capability negotiation beyond the current MVP contract surface;
- user-facing client SDKs;
- runtime orchestration;
- application state management;
- distributed consensus;
- business policy.

## Mechanism vs Policy

LAIC provides mechanisms that other systems may use. It does not decide policy
for those systems.

For example:

- LAIC may carry a message, but it does not decide which provider should receive it.
- LAIC may expose transport security primitives, but it does not define application authorization policy.
- LAIC may validate protocol shape, but it does not make runtime placement decisions.
- LAIC may provide a minimal trust-domain handshake, but it does not define long-lived session policy.

This separation is intentional. Expanding LAIC into runtime policy would make the
protocol less reusable and would blur responsibility between independent system
layers.

## Current MVP Scope

The current MVP stable surface is intentionally small. Public compatibility is
defined by:

- `README.md`;
- `docs/STABILITY.md`;
- `CHANGELOG.md`;
- crate-level documentation for `laic` and `laicc`;
- the published package metadata.

If a repository item is visible but not listed as stable in `docs/STABILITY.md`,
treat it as internal, experimental, or test/support material.

## Explicit Non-Goals

The current MVP does not promise:

- runtime SDKs;
- provider hosting;
- discovery or routing;
- session policy;
- retry or reconnect convenience layers;
- capability negotiation;
- gateway-hosted runtime behavior;
- stable semantics for early malformed handshakes that are outside the current
  ratified handshake boundary;
- token/session semantics that reinterpret handshake freshness markers as
  authentication secrets.

## Future Expansion

Future LAIC work may expand the stable surface, but expansion must be explicit.
Any material expansion requires:

- a documented design boundary;
- stable-surface documentation updates;
- changelog updates;
- independent review appropriate to the scope;
- compatibility impact analysis.

No downstream runtime, provider, or application behavior becomes part of LAIC
only because it is useful or adjacent.
