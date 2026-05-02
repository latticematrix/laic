//! TypeScript deserialization helpers.
//!
//! WHY: deserialization is where TS must re-assert the LAIC contract. If this path
//! becomes weaker than Rust/Python, cross-language roundtrips can silently drift.

use crate::ast::{Dimension, FieldDef, LaicType, StructDef, TensorElementType};
use crate::codegen::typescript_string_literal;
use crate::codegen::typescript_types::{format_ts_dims, literal_to_ts};

/// Emit `fromIpc()` for a generated TypeScript contract class.
pub fn generate_from_ipc(
    out: &mut String,
    def: &StructDef,
    skill_id: &str,
    version: &str,
    direction: &str,
) {
    out.push_str(&format!(
        "  static fromIpc(data: Uint8Array): {} {{\n",
        def.name
    ));
    out.push_str("    const table = arrow.tableFromIPC(data);\n");
    // Generated contracts always encode exactly one logical record. Cardinality checks
    // stay explicit instead of assuming "the first row is good enough".
    out.push_str("    if (table.numRows === 0) {\n");
    out.push_str(
        "      throw new Error(\"cardinality error: RecordBatch has 0 rows, expected 1\");\n",
    );
    out.push_str("    }\n");
    out.push_str("    if (table.numRows > 1) {\n");
    out.push_str("      throw new Error(`cardinality error: RecordBatch has ${table.numRows} rows, expected 1`);\n");
    out.push_str("    }\n");
    // Match Rust/Python: a single logical row is still invalid if the IPC stream carries
    // trailing RecordBatches. We reject on batch cardinality before reading row 0.
    out.push_str("    if (table.batches.length > 1) {\n");
    out.push_str(
        "      throw new Error(\"cardinality error: stream contains more than one RecordBatch\");\n",
    );
    out.push_str("    }\n");
    out.push_str("    const batch = table.batches[0]!;\n");
    out.push_str("    const schemaMetadata = laicSchemaMetadata(table.schema);\n");
    out.push_str(&format!(
        "    laicAssertMetadata(schemaMetadata, \"laic.skill_id\", {});\n",
        typescript_string_literal(skill_id)
    ));
    out.push_str(&format!(
        "    laicAssertMetadata(schemaMetadata, \"laic.version\", {});\n",
        typescript_string_literal(version)
    ));
    out.push_str(&format!(
        "    laicAssertMetadata(schemaMetadata, \"laic.direction\", \"{direction}\");\n\n"
    ));

    for field in &def.fields {
        emit_field_extraction(out, field);
    }

    out.push_str(&format!("    return new {}(\n", def.name));
    for field in &def.fields {
        out.push_str(&format!("      {},\n", field.name));
    }
    out.push_str("    );\n");
    out.push_str("  }\n");
}

fn emit_field_extraction(out: &mut String, field: &FieldDef) {
    let name = &field.name;
    match &field.ty {
        LaicType::Tensor { .. } => {
            if let LaicType::Tensor { dtype, dims } = &field.ty {
                // Tensor bytes alone are not enough: dtype/shape metadata is part of the
                // wire contract and must be enforced here to stay aligned with Rust/Python.
                emit_tensor_metadata_assertion(out, name, dtype, dims);
            }
            out.push_str(&format!(
                "    const {name} = batch.getChild(\"{name}\")!.get(0) as Uint8Array;\n"
            ));
        }
        LaicType::List(inner) => match inner.as_ref() {
            LaicType::Tensor { .. } => {
                if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                    emit_tensor_metadata_assertion(out, name, dtype, dims);
                }
                out.push_str(&format!(
                    "    const {name} = Array.from(batch.getChild(\"{name}\")!.get(0) as Iterable<Uint8Array>);\n"
                ));
            }
            _ => {
                out.push_str(&format!(
                    "    const {name} = Array.from(batch.getChild(\"{name}\")!.get(0) as Iterable<{}>);\n",
                    crate::codegen::typescript_types::ts_type(inner)
                ));
            }
        },
        LaicType::Optional(inner) => match inner.as_ref() {
            LaicType::Tensor { .. } => {
                if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                    // Nullable tensor fields still need metadata validation before we
                    // inspect row validity; otherwise `null` would accidentally disable
                    // contract checks for the field definition itself.
                    emit_tensor_metadata_assertion(out, name, dtype, dims);
                }
                out.push_str(&format!(
                    "    const {name}_column = batch.getChild(\"{name}\")!;\n"
                ));
                out.push_str(&format!(
                    "    const {name} = {name}_column.isValid(0) ? ({name}_column.get(0) as Uint8Array) : null;\n"
                ));
            }
            _ => {
                out.push_str(&format!(
                    "    const {name}_column = batch.getChild(\"{name}\")!;\n"
                ));
                out.push_str(&format!(
                    "    const {name} = {name}_column.isValid(0) ? ({name}_column.get(0) as {}) : null;\n",
                    crate::codegen::typescript_types::ts_type(inner)
                ));
            }
        },
        LaicType::Map(key, value) => {
            out.push_str(&format!(
                "    const {name} = batch.getChild(\"{name}\")!.get(0) as Map<{}, {}>;\n",
                crate::codegen::typescript_types::ts_type(key),
                crate::codegen::typescript_types::ts_type(value)
            ));
        }
        _ => match &field.default {
            Some(default) => {
                out.push_str(&format!(
                    "    const {name}_column = batch.getChild(\"{name}\");\n"
                ));
                out.push_str(&format!(
                    "    const {name} = {name}_column === null ? {} : ({name}_column.get(0) as {});\n",
                    literal_to_ts(default),
                    crate::codegen::typescript_types::ts_type(&field.ty)
                ));
            }
            None => {
                out.push_str(&format!(
                    "    const {name} = batch.getChild(\"{name}\")!.get(0) as {};\n",
                    crate::codegen::typescript_types::ts_type(&field.ty)
                ));
            }
        },
    }
}

fn emit_tensor_metadata_assertion(
    out: &mut String,
    field_name: &str,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    out.push_str(&format!(
        "    const {field_name}_field = table.schema.fields.find((candidate) => candidate.name === \"{field_name}\")!;\n"
    ));
    out.push_str(&format!(
        "    laicAssertTensorMetadata({field_name}_field, \"{field_name}\", \"{}\", {});\n",
        dtype.as_str(),
        format_ts_dims(dims)
    ));
}
