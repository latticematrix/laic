//! Generate Python `to_ipc()` serialization methods.

use crate::ast::{Dimension, FieldDef, LaicType, SkillDef, StructDef, TensorElementType};
use crate::codegen::python_bytes_literal;
use crate::codegen::python_types::pyarrow_type;

/// Emit `to_ipc(self) -> bytes` method body.
pub fn generate_to_ipc(
    out: &mut String,
    skill: &SkillDef,
    def: &StructDef,
    direction: &str,
    version: &str,
) {
    out.push_str("    def to_ipc(self) -> bytes:\n");
    out.push_str("        \"\"\"Serialize to Arrow IPC stream format.\"\"\"\n");

    // Schema fields
    out.push_str("        schema = pa.schema([\n");
    for field in &def.fields {
        emit_schema_field(out, field);
    }
    out.push_str("        ], metadata={\n");
    out.push_str(&format!(
        "            b\"laic.skill_id\": {},\n",
        python_bytes_literal(&skill.id)
    ));
    out.push_str(&format!(
        "            b\"laic.version\": {},\n",
        python_bytes_literal(version)
    ));
    out.push_str(&format!(
        "            b\"laic.direction\": b\"{direction}\",\n"
    ));
    out.push_str("        })\n");

    // Build pydict
    out.push_str("        batch = pa.RecordBatch.from_pydict({\n");
    for field in &def.fields {
        emit_pydict_value(out, &field.name, &field.ty);
    }
    out.push_str("        }, schema=schema)\n");

    // Write IPC
    out.push_str("        sink = pa.BufferOutputStream()\n");
    out.push_str("        writer = ipc.new_stream(sink, schema)\n");
    out.push_str("        writer.write_batch(batch)\n");
    out.push_str("        writer.close()\n");
    out.push_str("        return sink.getvalue().to_pybytes()\n");
    out.push('\n');
}

fn emit_schema_field(out: &mut String, field: &FieldDef) {
    let name = &field.name;
    match &field.ty {
        LaicType::Tensor { dtype, dims } => {
            out.push_str(&format!(
                "            pa.field(\"{name}\", pa.binary(), nullable=False, metadata={{\n"
            ));
            emit_tensor_metadata(out, dtype, dims);
            out.push_str("            }),\n");
        }
        LaicType::Optional(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                out.push_str(&format!(
                    "            pa.field(\"{name}\", pa.binary(), nullable=True, metadata={{\n"
                ));
                emit_tensor_metadata(out, dtype, dims);
                out.push_str("            }),\n");
            }
        }
        LaicType::List(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                // WHY: list<tensor> uses list_(binary()) with tensor metadata on the list field
                out.push_str(&format!(
                    "            pa.field(\"{name}\", pa.list_(pa.binary()), nullable=False, metadata={{\n"
                ));
                emit_tensor_metadata(out, dtype, dims);
                out.push_str("            }),\n");
            }
        }
        _ => {
            let nullable = matches!(&field.ty, LaicType::Optional(_));
            let pa_type = pyarrow_type(&field.ty);
            out.push_str(&format!(
                "            pa.field(\"{name}\", {pa_type}, nullable={nullable}),\n",
                nullable = if nullable { "True" } else { "False" },
            ));
        }
    }
}

fn emit_tensor_metadata(out: &mut String, dtype: &TensorElementType, dims: &[Dimension]) {
    out.push_str(&format!(
        "                b\"laic.tensor.dtype\": b\"{}\",\n",
        dtype.as_str()
    ));
    let shape = format_dims_as_shape(dims);
    out.push_str(&format!(
        "                b\"laic.tensor.shape\": b\"{shape}\",\n"
    ));
    out.push_str("                b\"laic.tensor.version\": b\"1\",\n");
}

fn format_dims_as_shape(dims: &[Dimension]) -> String {
    let parts: Vec<String> = dims
        .iter()
        .map(|d| match d {
            Dimension::Fixed(n) => n.to_string(),
            Dimension::Dynamic(_) => "0".into(),
        })
        .collect();
    format!("[{}]", parts.join(","))
}

/// Emit the pydict value expression for a field.
fn emit_pydict_value(out: &mut String, name: &str, ty: &LaicType) {
    // WHY: PyArrow from_pydict requires values wrapped in a list (one row per batch element).
    // Scalars: [self.x], List/Map: [self.x] (the outer list is the batch dimension).
    match ty {
        LaicType::Map(_, _) => {
            // WHY: pa.map_ expects list of list-of-tuples for from_pydict
            out.push_str(&format!(
                "            \"{name}\": [list(self.{name}.items())],\n"
            ));
        }
        _ => {
            out.push_str(&format!("            \"{name}\": [self.{name}],\n"));
        }
    }
}
