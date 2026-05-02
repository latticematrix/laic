use super::python_fixture::python_driver_script;

pub(crate) fn python_driver(stem: &str) -> String {
    python_driver_script(
        stem,
        &format!(
            r#"import json
import os
import pathlib
import sys

import pyarrow as pa
import pyarrow.ipc as ipc

CONFIG = json.loads(os.environ["LAIC_COMPAT_CONFIG"])

def materialize(value):
    if isinstance(value, dict) and "__bytes_len__" in value:
        return b"\x00" * int(value["__bytes_len__"])
    if isinstance(value, list):
        return [materialize(item) for item in value]
    if isinstance(value, dict):
        return {{key: materialize(item) for key, item in value.items()}}
    return value

def normalize(value):
    if isinstance(value, (bytes, bytearray, memoryview)):
        return {{"kind": "bytes", "len": len(value)}}
    if isinstance(value, list):
        return [normalize(item) for item in value]
    if isinstance(value, dict):
        return {{key: normalize(item) for key, item in value.items()}}
    return value

def build(kind):
    cls = globals()[CONFIG[f"{{kind}}_class"]]
    args = [materialize(value) for value in CONFIG[f"{{kind}}_args"]]
    return cls(*args)

def normalize_instance(instance, kind):
    return {{name: normalize(getattr(instance, name)) for name in CONFIG[f"{{kind}}_fields"]}}

def inspect_table(data):
    return ipc.open_stream(pa.py_buffer(data)).read_all()

def rejects_multiple_batches(cls, data):
    table = inspect_table(data)
    first = table.to_batches()[0]
    empty = first.slice(1, 0)
    sink = pa.BufferOutputStream()
    writer = ipc.new_stream(sink, table.schema)
    writer.write_batch(first)
    writer.write_batch(empty)
    writer.close()
    try:
        cls.from_ipc(sink.getvalue().to_pybytes())
        return False
    except Exception:
        return True

def rejects_tensor_dtype_mismatch(cls, data, kind):
    field_name = CONFIG.get("tensor_field")
    bad_dtype = CONFIG.get("bad_tensor_dtype")
    if kind != "output" or field_name is None or bad_dtype is None:
        return False
    table = inspect_table(data)
    fields = []
    for field in table.schema:
        if field.name == field_name:
            meta = dict(field.metadata or {{}})
            meta[b"laic.tensor.dtype"] = bad_dtype.encode()
            fields.append(pa.field(field.name, field.type, nullable=field.nullable, metadata=meta))
        else:
            fields.append(field)
    schema = pa.schema(fields, metadata=table.schema.metadata)
    drifted = pa.Table.from_arrays([table.column(i) for i in range(table.num_columns)], schema=schema)
    sink = pa.BufferOutputStream()
    writer = ipc.new_stream(sink, schema)
    writer.write_table(drifted)
    writer.close()
    try:
        cls.from_ipc(sink.getvalue().to_pybytes())
        return False
    except Exception:
        return True

def snapshot(kind):
    instance = build(kind)
    cls = type(instance)
    data = instance.to_ipc()
    table = inspect_table(data)
    restored = cls.from_ipc(data)
    metadata = {{key.decode(): value.decode() for key, value in (table.schema.metadata or {{}}).items()}}
    return {{
        "skill_id": cls.SKILL_ID,
        "version": cls.VERSION,
        "direction": cls.DIRECTION,
        "fields": normalize_instance(restored, kind),
        "schema_metadata": metadata,
        "row_count": table.num_rows,
        "record_batch_count": len(table.to_batches()),
        "rejects_multiple_batches": rejects_multiple_batches(cls, data),
        "rejects_tensor_dtype_mismatch": rejects_tensor_dtype_mismatch(cls, data, kind),
    }}

def errors():
    enum_name = CONFIG.get("error_enum")
    if enum_name is None:
        return {{}}
    enum_cls = globals()[enum_name]
    return {{name: int(member.value) for name, member in enum_cls.__members__.items()}}

def write_payloads(target):
    target.mkdir(parents=True, exist_ok=True)
    target.joinpath("input.ipc").write_bytes(build("input").to_ipc())
    target.joinpath("output.ipc").write_bytes(build("output").to_ipc())

def consume(input_path, output_path):
    input_cls = globals()[CONFIG["input_class"]]
    output_cls = globals()[CONFIG["output_class"]]
    input_restored = input_cls.from_ipc(pathlib.Path(input_path).read_bytes())
    output_restored = output_cls.from_ipc(pathlib.Path(output_path).read_bytes())
    return {{
        "input": normalize_instance(input_restored, "input"),
        "output": normalize_instance(output_restored, "output"),
    }}

mode = sys.argv[1]
if mode == "snapshot":
    print(json.dumps({{"input": snapshot("input"), "output": snapshot("output"), "errors": errors()}}))
elif mode == "produce":
    write_payloads(pathlib.Path(sys.argv[2]))
elif mode == "consume":
    print(json.dumps(consume(sys.argv[2], sys.argv[3])))
else:
    raise ValueError(f"unknown mode: {{mode}}")
"#
        ),
    )
}

pub(crate) fn typescript_driver() -> &'static str {
    r#"import * as fs from "node:fs";
import * as arrow from "apache-arrow";
import * as contract from "./index";

type CompatConfig = {
  input_class: string;
  output_class: string;
  input_fields: string[];
  output_fields: string[];
  input_args: unknown[];
  output_args: unknown[];
  error_enum?: string;
  tensor_field?: string;
  bad_tensor_dtype?: string;
};

const config = JSON.parse(process.env.LAIC_COMPAT_CONFIG ?? "") as CompatConfig;

function materialize(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(materialize);
  }
  if (value && typeof value === "object" && "__bytes_len__" in value) {
    return new Uint8Array(Number((value as { __bytes_len__: number }).__bytes_len__));
  }
  return value;
}

function normalize(value: unknown): unknown {
  if (value instanceof Uint8Array) {
    return { kind: "bytes", len: value.length };
  }
  if (Array.isArray(value)) {
    return value.map(normalize);
  }
  if (value instanceof Map) {
    return Object.fromEntries([...value.entries()].map(([key, entry]) => [String(key), normalize(entry)]));
  }
  return value;
}

function ctor(kind: "input" | "output"): any {
  return (contract as Record<string, unknown>)[kind === "input" ? config.input_class : config.output_class] as any;
}

function build(kind: "input" | "output"): any {
  const args = (kind === "input" ? config.input_args : config.output_args).map(materialize);
  return new (ctor(kind))(...args);
}

function normalizeInstance(instance: Record<string, unknown>, kind: "input" | "output"): Record<string, unknown> {
  const fields = kind === "input" ? config.input_fields : config.output_fields;
  return Object.fromEntries(fields.map((name) => [name, normalize(instance[name])]));
}

function schemaMetadata(table: arrow.Table): Record<string, string> {
  return Object.fromEntries([...(table.schema.metadata ?? new Map()).entries()]);
}

function rejectsMultipleBatches(cls: any, data: Uint8Array): boolean {
  const table = arrow.tableFromIPC(data);
  const firstBatch = table.batches[0]!;
  const trailingEmptyBatch = firstBatch.slice(1, 1);
  const driftedTable = new arrow.Table(table.schema, [firstBatch, trailingEmptyBatch]);
  try {
    cls.fromIpc(arrow.tableToIPC(driftedTable));
    return false;
  } catch {
    return true;
  }
}

function rejectsTensorDtypeMismatch(cls: any, data: Uint8Array, kind: "input" | "output"): boolean {
  if (kind !== "output" || !config.tensor_field || !config.bad_tensor_dtype) {
    return false;
  }
  const table = arrow.tableFromIPC(data);
  const fields = table.schema.fields.map((field) => {
    if (field.name !== config.tensor_field) {
      return field;
    }
    const metadata = new Map((field.metadata ?? new Map()).entries());
    metadata.set("laic.tensor.dtype", config.bad_tensor_dtype!);
    return new arrow.Field(field.name, field.type, field.nullable, metadata);
  });
  const columns: Record<string, arrow.Vector> = {};
  for (const field of table.schema.fields) {
    columns[field.name] = table.getChild(field.name)!;
  }
  try {
    cls.fromIpc(arrow.tableToIPC(new arrow.Table(new arrow.Schema(fields, table.schema.metadata ?? new Map()), columns as any)));
    return false;
  } catch {
    return true;
  }
}

function snapshot(kind: "input" | "output"): Record<string, unknown> {
  const instance = build(kind);
  const cls = ctor(kind);
  const data = instance.toIpc() as Uint8Array;
  const table = arrow.tableFromIPC(data);
  const restored = cls.fromIpc(data) as Record<string, unknown>;
  return {
    skill_id: cls.SKILL_ID,
    version: cls.VERSION,
    direction: cls.DIRECTION,
    fields: normalizeInstance(restored, kind),
    schema_metadata: schemaMetadata(table),
    row_count: table.numRows,
    record_batch_count: table.batches.length,
    rejects_multiple_batches: rejectsMultipleBatches(cls, data),
    rejects_tensor_dtype_mismatch: rejectsTensorDtypeMismatch(cls, data, kind),
  };
}

function errors(): Record<string, number> {
  if (!config.error_enum) {
    return {};
  }
  const raw = (contract as Record<string, unknown>)[config.error_enum] as Record<string, number | string>;
  return Object.fromEntries(Object.entries(raw).filter(([, value]) => Number.isInteger(value))) as Record<string, number>;
}

function writePayloads(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(`${dir}/input.ipc`, Buffer.from(build("input").toIpc() as Uint8Array));
  fs.writeFileSync(`${dir}/output.ipc`, Buffer.from(build("output").toIpc() as Uint8Array));
}

function consume(inputPath: string, outputPath: string): Record<string, unknown> {
  const inputRestored = ctor("input").fromIpc(fs.readFileSync(inputPath)) as Record<string, unknown>;
  const outputRestored = ctor("output").fromIpc(fs.readFileSync(outputPath)) as Record<string, unknown>;
  return {
    input: normalizeInstance(inputRestored, "input"),
    output: normalizeInstance(outputRestored, "output"),
  };
}

const mode = process.argv[2];
if (mode === "snapshot") {
  process.stdout.write(JSON.stringify({ input: snapshot("input"), output: snapshot("output"), errors: errors() }));
} else if (mode === "produce") {
  writePayloads(process.argv[3]!);
} else if (mode === "consume") {
  process.stdout.write(JSON.stringify(consume(process.argv[3]!, process.argv[4]!)));
} else {
  throw new Error(`unknown mode: ${mode}`);
}
"#
}
