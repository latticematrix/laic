# Python Verify Runtime Fixture

This directory exists only for `laicc` Python verify reproducibility.

- It is not a Python runtime SDK.
- It is not a product-facing package surface.
- It only pins the dependency set used by `crates/laicc/tests/python_verify.rs` and CI.

Install the fixture dependencies with:

```powershell
python -m pip install -r crates/laicc/tests/python_runtime/requirements.txt
```

Why `pyarrow` is pinned:

- Python verify exercises generated contract modules against Arrow IPC behavior.
- A pinned version keeps local runs and CI on the same baseline.
- This fixture is intentionally minimal so Phase 8 stays inside the LAIC contract surface boundary.
