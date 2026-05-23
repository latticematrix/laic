//! Generate `from_arrow_ipc()` deserialization methods.

use crate::ast::{FieldDef, LaicType, StructDef};
use crate::codegen::rust_deserialize_fields::{
    emit_list_extraction, emit_map_extraction, emit_optional_extraction, emit_tensor_extraction,
    primitive_value_expr,
};
use crate::codegen::rust_string_literal;
use crate::codegen::rust_types::arrow_array_type;

/// Emit `from_arrow_ipc(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>>`.
pub fn generate_from_arrow_ipc(
    out: &mut String,
    def: &StructDef,
    skill_id: &str,
    version: &str,
    direction: &str,
) {
    out.push_str(
        "    pub fn from_arrow_ipc(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {\n",
    );
    out.push_str("        let cursor = std::io::Cursor::new(bytes);\n");
    out.push_str(
        "        let mut reader = arrow_ipc::reader::StreamReader::try_new(cursor, None)?;\n",
    );
    out.push_str("        let batch = reader.next()\n");
    out.push_str("            .ok_or(\"empty IPC stream: no RecordBatch\")?\n");
    out.push_str("            ?;\n");

    // Cardinality: exactly 1 row
    out.push_str("        if batch.num_rows() == 0 {\n");
    out.push_str(
        "            return Err(\"cardinality error: RecordBatch has 0 rows, expected 1\".into());\n",
    );
    out.push_str("        }\n");
    out.push_str("        if batch.num_rows() > 1 {\n");
    out.push_str("            return Err(format!(\"cardinality error: RecordBatch has {} rows, expected 1\", batch.num_rows()).into());\n");
    out.push_str("        }\n");

    // Trailing batch check: no more batches allowed
    out.push_str("        match reader.next() {\n");
    out.push_str("            Some(Ok(_)) => return Err(\"cardinality error: stream contains more than one RecordBatch\".into()),\n");
    out.push_str("            Some(Err(e)) => return Err(format!(\"corrupt trailing data in IPC stream: {e}\").into()),\n");
    out.push_str("            None => {}\n");
    out.push_str("        }\n");

    // Metadata validation
    emit_metadata_validation(out, skill_id, version, direction);

    // Extract each field
    for field in &def.fields {
        emit_field_extraction(out, field);
    }

    // Build struct
    out.push_str("        Ok(Self {\n");
    for field in &def.fields {
        out.push_str(&format!("            {},\n", field.name));
    }
    out.push_str("        })\n");
    out.push_str("    }\n");
}

fn emit_metadata_validation(out: &mut String, skill_id: &str, version: &str, direction: &str) {
    out.push_str("        let schema = batch.schema();\n");
    out.push_str("        let meta = schema.metadata();\n");

    for (key, expected) in [
        ("laic.skill_id", skill_id),
        ("laic.version", version),
        ("laic.direction", direction),
    ] {
        out.push_str(&format!("        match meta.get(\"{key}\") {{\n"));
        out.push_str(&format!(
            "            None => return Err(\"missing required metadata key '{}'\".into()),\n",
            key
        ));
        out.push_str(&format!(
            "            Some(v) if v != {} => return Err(format!(\"metadata '{}' mismatch: expected {{:?}}, got '{{}}'\", {}, v).into()),\n",
            rust_string_literal(expected),
            key,
            rust_string_literal(expected)
        ));
        out.push_str("            _ => {}\n");
        out.push_str("        }\n");
    }
}

fn emit_field_extraction(out: &mut String, field: &FieldDef) {
    let name = &field.name;

    match &field.ty {
        LaicType::Tensor { dtype, dims } => {
            emit_tensor_extraction(out, name, dtype, dims);
        }
        LaicType::Bytes => {
            let expected_type = crate::codegen::rust_types::arrow_datatype(&field.ty);
            emit_simple_extraction(
                out,
                name,
                "BinaryArray",
                ".value(0).to_vec()",
                &expected_type,
                &field.default,
            );
        }
        LaicType::List(inner) => {
            emit_list_extraction(out, name, inner);
        }
        LaicType::Optional(inner) => {
            emit_optional_extraction(out, name, inner);
        }
        LaicType::Map(k, v) => {
            emit_map_extraction(out, name, k, v);
        }
        _ => {
            let arr_type = arrow_array_type(&field.ty);
            let val_expr = primitive_value_expr(&field.ty);
            let expected_type = crate::codegen::rust_types::arrow_datatype(&field.ty);
            emit_simple_extraction(
                out,
                name,
                arr_type,
                val_expr,
                &expected_type,
                &field.default,
            );
        }
    }
}

fn emit_simple_extraction(
    out: &mut String,
    name: &str,
    array_type: &str,
    value_expr: &str,
    expected_type: &str,
    default: &Option<crate::ast::Literal>,
) {
    match default {
        Some(lit) => {
            let default_val = crate::codegen::rust_types::literal_to_rust(lit);
            out.push_str(&format!(
                "        let {name} = match batch.column_by_name(\"{name}\") {{\n"
            ));
            out.push_str("            Some(column) => {\n");
            out.push_str("                let schema = batch.schema();\n");
            out.push_str(&format!(
                "                let field = schema.field_with_name(\"{name}\")\n"
            ));
            out.push_str(&format!(
                "                    .map_err(|_| \"missing '{}' field\".to_string())?;\n",
                name
            ));
            out.push_str(&format!(
                "                if field.data_type() != &{expected_type} {{\n"
            ));
            out.push_str(&format!(
                "                    return Err(format!(\"field '{}' expected {{:?}}, got {{:?}}\", {expected_type}, field.data_type()).into());\n",
                name
            ));
            out.push_str("                }\n");
            out.push_str(&format!(
                "                column\n                    .as_any()\n                    .downcast_ref::<{array_type}>()\n"
            ));
            out.push_str(&format!(
                "                    .ok_or_else(|| \"field '{}' expected {array_type}\".to_string())?\n",
                name
            ));
            out.push_str(&format!("                    {value_expr}\n"));
            out.push_str("            }\n");
            out.push_str(&format!("            None => {default_val},\n"));
            out.push_str("        };\n");
        }
        None => {
            out.push_str(&format!(
                "        let {name} = match batch.column_by_name(\"{name}\") {{\n"
            ));
            out.push_str("            Some(column) => {\n");
            out.push_str("                let schema = batch.schema();\n");
            out.push_str(&format!(
                "                let field = schema.field_with_name(\"{name}\")\n"
            ));
            out.push_str(&format!(
                "                    .map_err(|_| \"missing '{}' field\".to_string())?;\n",
                name
            ));
            out.push_str(&format!(
                "                if field.data_type() != &{expected_type} {{\n"
            ));
            out.push_str(&format!(
                "                    return Err(format!(\"field '{}' expected {{:?}}, got {{:?}}\", {expected_type}, field.data_type()).into());\n",
                name
            ));
            out.push_str("                }\n");
            out.push_str(&format!(
                "                column\n                    .as_any()\n                    .downcast_ref::<{array_type}>()\n"
            ));
            out.push_str(&format!(
                "                    .ok_or_else(|| \"field '{}' expected {array_type}\".to_string())?\n",
                name
            ));
            out.push_str(&format!("                    {value_expr}\n"));
            out.push_str("            }\n");
            out.push_str(&format!(
                "            None => return Err(\"missing '{}' column\".into()),\n",
                name
            ));
            out.push_str("        };\n");
        }
    }
}
