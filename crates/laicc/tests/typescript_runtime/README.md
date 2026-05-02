# TypeScript Verify Runtime Fixture

This directory exists only for `laicc` TypeScript verify reproducibility.

- It is not a TypeScript runtime SDK.
- It only proves generated contract modules are consumable through a minimal package surface.
- It keeps verify-local dependencies and package-root layout in one repo-local fixture.

Install the fixture dependencies with:

```powershell
npm ci --prefix crates/laicc/tests/typescript_runtime
```
