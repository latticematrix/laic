//! TypeScript serialization helpers.
//!
//! WHY: the generated surface favors explicit Arrow schema construction over hidden
//! helpers so reviewers can see exactly which LAIC metadata is put on the wire.

use crate::ast::{Dimension, LaicType, SkillDef, StructDef, TensorElementType};
use crate::codegen::typescript_string_literal;
use crate::codegen::typescript_types::{format_ts_dims, ts_arrow_datatype};

/// Emit `toIpc()` for a generated TypeScript contract class.
pub fn generate_to_ipc(
    out: &mut String,
    skill: &SkillDef,
    def: &StructDef,
    direction: &str,
    version: &str,
) {
    out.push_str("  toIpc(): Uint8Array {\n");
    out.push_str("    const schema = new arrow.Schema([\n");
    for field in &def.fields {
        emit_schema_field(out, field);
    }
    out.push_str("    ], new Map([\n");
    out.push_str(&format!(
        "      [\"laic.skill_id\", {}],\n",
        typescript_string_literal(&skill.id)
    ));
    out.push_str(&format!(
        "      [\"laic.version\", {}],\n",
        typescript_string_literal(version)
    ));
    out.push_str(&format!("      [\"laic.direction\", \"{direction}\"],\n"));
    out.push_str("    ]));\n\n");

    out.push_str("    const data: Record<string, unknown[]> = {\n");
    for field in &def.fields {
        out.push_str(&format!(
            "      \"{}\": [this.{}],\n",
            field.name, field.name
        ));
    }
    out.push_str("    };\n\n");

    out.push_str("    const columns: Record<string, arrow.Vector> = {};\n");
    out.push_str("    for (const field of schema.fields) {\n");
    out.push_str(
        "      columns[field.name] = arrow.vectorFromArray(data[field.name], field.type);\n",
    );
    out.push_str("    }\n");
    // Arrow JS 21 accepts this construction path at runtime, but its type surface is
    // stricter than the actual API. Keep the cast local here instead of spreading `any`
    // through the generated class bodies.
    out.push_str("    const table = new arrow.Table(schema, columns as any);\n");
    out.push_str("    return arrow.tableToIPC(table);\n");
    out.push_str("  }\n\n");
}

fn emit_schema_field(out: &mut String, field: &crate::ast::FieldDef) {
    let nullable = matches!(field.ty, LaicType::Optional(_));
    match &field.ty {
        LaicType::Tensor { dtype, dims } => {
            // Tensor payloads stay bytes-based in TS for now. The semantic contract is
            // carried by field metadata, not by a higher-level tensor runtime wrapper.
            emit_tensor_schema_field(out, &field.name, "new arrow.Binary()", false, dtype, dims);
        }
        LaicType::List(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                // list<tensor> keeps per-field tensor metadata on the outer field so the
                // consumer can validate the entire collection with one invariant check.
                emit_tensor_schema_field(
                    out,
                    &field.name,
                    "new arrow.List(new arrow.Field(\"item\", new arrow.Binary(), false))",
                    false,
                    dtype,
                    dims,
                );
            }
        }
        LaicType::Optional(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                // optional<tensor> still publishes full tensor metadata even when the
                // first value is null; otherwise a nullable field would weaken contract
                // validation relative to the non-optional tensor path.
                emit_tensor_schema_field(out, &field.name, "new arrow.Binary()", true, dtype, dims);
            }
        }
        _ => {
            out.push_str(&format!(
                "      new arrow.Field(\"{}\", {}, {}),\n",
                field.name,
                ts_arrow_datatype(&field.ty),
                nullable
            ));
        }
    }
}

fn emit_tensor_schema_field(
    out: &mut String,
    field_name: &str,
    arrow_type: &str,
    nullable: bool,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    out.push_str(&format!(
        "      new arrow.Field(\"{field_name}\", {arrow_type}, {}, new Map([\n",
        if nullable { "true" } else { "false" }
    ));
    out.push_str(&format!(
        "        [\"laic.tensor.dtype\", \"{}\"],\n",
        dtype.as_str()
    ));
    out.push_str(&format!(
        "        [\"laic.tensor.shape\", \"{}\"],\n",
        format_ts_dims(dims)
    ));
    out.push_str("        [\"laic.tensor.version\", \"1\"],\n");
    out.push_str("      ])),\n");
}
