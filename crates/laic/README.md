# laic

`laic` is the mechanism-layer Rust crate in the LAIC MVP.
It provides transport, protocol, flow-control, emergency-channel, gateway, and minimal trust-domain handshake mechanisms for AI system communication.
It does not define runtime policy, discovery, routing, provider hosting, or client convenience layers.

## Install

```powershell
cargo add laic
```

## Stable Surface Summary

The current MVP stable surface is the documented crate-root API centered on:

- transport entry points such as `Transport`, `IpcConnection`, `QuicConnection`, and `QuicServer`
- protocol and message types such as `MessageHeader`, `Message`, `MsgType`, `PayloadFormat`, and `Qos`
- published error-code and error-type surfaces such as `ErrorCode`, `LaicError`, `TransportError`, `CodecError`, `ProtocolError`, and `FlowError`
- mechanism-layer helpers such as `CreditController`, `EmergencyChannel`, and `Gateway`
- the minimal trust-domain handshake surface, including `client_handshake`, `server_handshake`, and `TrustDomainSession`

Module topology below crate root is not a separate compatibility promise just because an item is currently reachable through a public module path.

## What This Package Does Not Promise

This package does not promise:

- runtime SDKs
- discovery or routing
- provider hosting
- session policy
- retry or reconnect convenience layers
- internal module layout or repo-local test/support code as stable API

## Release-Facing References

- Repository: <https://github.com/latticematrix/laic>
- Boundary: <https://github.com/latticematrix/laic/blob/main/docs/BOUNDARY.md>
- Stability contract: <https://github.com/latticematrix/laic/blob/main/docs/STABILITY.md>
- Changelog: <https://github.com/latticematrix/laic/blob/main/CHANGELOG.md>

## License

Licensed under the Apache License, Version 2.0.
