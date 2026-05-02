//! Generate `to_arrow_ipc()` serialization methods.

use crate::ast::{Dimension, FieldDef, LaicType, SkillDef, StructDef, TensorElementType};
use crate::codegen::rust_string_literal;
use crate::codegen::rust_types::{arrow_builder_type, arrow_datatype, needs_deref};

/// Emit `to_arrow_ipc(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>` for a struct.
pub fn generate_to_arrow_ipc(
    out: &mut String,
    skill: &SkillDef,
    def: &StructDef,
    direction: &str,
    version: &str,
) {
    out.push_str(
        "    pub fn to_arrow_ipc(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {\n",
    );

    // Schema fields
    out.push_str("        let schema = Schema::new_with_metadata(\n");
    out.push_str("            vec![\n");
    for field in &def.fields {
        emit_schema_field(out, field);
    }
    out.push_str("            ],\n");

    // Schema-level metadata
    out.push_str("            HashMap::from([\n");
    out.push_str(&format!(
        "                (\"laic.skill_id\".into(), {}.into()),\n",
        rust_string_literal(&skill.id)
    ));
    out.push_str(&format!(
        "                (\"laic.version\".into(), {}.into()),\n",
        rust_string_literal(version)
    ));
    out.push_str(&format!(
        "                (\"laic.direction\".into(), \"{direction}\".into()),\n",
    ));
    out.push_str("            ]),\n");
    out.push_str("        );\n");

    // Build columns
    out.push_str("        let batch = RecordBatch::try_new(Arc::new(schema), vec![\n");
    for field in &def.fields {
        out.push_str("            ");
        emit_to_arrow_column(&field.name, &field.ty, out);
        out.push_str(",\n");
    }
    out.push_str("        ])?;\n");

    // Write IPC
    out.push_str("        let mut buf = Vec::new();\n");
    out.push_str("        {\n");
    out.push_str(
        "            let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut buf, &batch.schema())?;\n",
    );
    out.push_str("            writer.write(&batch)?;\n");
    out.push_str("            writer.finish()?;\n");
    out.push_str("        }\n");
    out.push_str("        Ok(buf)\n");
    out.push_str("    }\n");
}

fn emit_schema_field(out: &mut String, field: &FieldDef) {
    match &field.ty {
        LaicType::Tensor { dtype, dims } => {
            out.push_str(&format!(
                "                Field::new(\"{}\", DataType::Binary, false)\n",
                field.name
            ));
            emit_tensor_field_metadata(out, dtype, dims);
        }
        LaicType::List(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                // WHY: ListBuilder produces nullable inner field; schema must match
                out.push_str(&format!(
                    "                Field::new(\"{}\", DataType::List(Arc::new(Field::new(\"item\", DataType::Binary, true))), false)\n",
                    field.name
                ));
                emit_tensor_field_metadata(out, dtype, dims);
            }
        }
        LaicType::Optional(inner) if matches!(inner.as_ref(), LaicType::Tensor { .. }) => {
            if let LaicType::Tensor { dtype, dims } = inner.as_ref() {
                out.push_str(&format!(
                    "                Field::new(\"{}\", DataType::Binary, true)\n",
                    field.name
                ));
                emit_tensor_field_metadata(out, dtype, dims);
            }
        }
        _ => {
            let nullable = matches!(&field.ty, LaicType::Optional(_));
            out.push_str(&format!(
                "                Field::new(\"{}\", {}, {nullable}),\n",
                field.name,
                arrow_datatype(&field.ty),
            ));
        }
    }
}

fn emit_tensor_field_metadata(out: &mut String, dtype: &TensorElementType, dims: &[Dimension]) {
    out.push_str("                    .with_metadata(HashMap::from([\n");
    out.push_str(&format!(
        "                        (\"laic.tensor.dtype\".into(), \"{}\".into()),\n",
        dtype.as_str()
    ));
    let shape = format_dims_as_shape(dims);
    out.push_str(&format!(
        "                        (\"laic.tensor.shape\".into(), \"{shape}\".into()),\n",
    ));
    out.push_str("                        (\"laic.tensor.version\".into(), \"1\".into()),\n");
    out.push_str("                    ])),\n");
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

fn emit_to_arrow_column(name: &str, ty: &LaicType, out: &mut String) {
    match ty {
        LaicType::String => {
            out.push_str(&format!(
                "Arc::new(StringArray::from(vec![self.{name}.as_str()]))"
            ));
        }
        LaicType::Bytes => {
            out.push_str(&format!(
                "Arc::new(BinaryArray::from(vec![self.{name}.as_slice()]))"
            ));
        }
        LaicType::Bool => {
            out.push_str(&format!("Arc::new(BooleanArray::from(vec![self.{name}]))"));
        }
        LaicType::I8 => {
            out.push_str(&format!("Arc::new(Int8Array::from(vec![self.{name}]))"));
        }
        LaicType::I16 => {
            out.push_str(&format!("Arc::new(Int16Array::from(vec![self.{name}]))"));
        }
        LaicType::I32 => {
            out.push_str(&format!("Arc::new(Int32Array::from(vec![self.{name}]))"));
        }
        LaicType::I64 => {
            out.push_str(&format!("Arc::new(Int64Array::from(vec![self.{name}]))"));
        }
        LaicType::U8 => {
            out.push_str(&format!("Arc::new(UInt8Array::from(vec![self.{name}]))"));
        }
        LaicType::F32 => {
            out.push_str(&format!("Arc::new(Float32Array::from(vec![self.{name}]))"));
        }
        LaicType::F64 => {
            out.push_str(&format!("Arc::new(Float64Array::from(vec![self.{name}]))"));
        }
        LaicType::Tensor { .. } => {
            out.push_str(&format!(
                "Arc::new(BinaryArray::from(vec![self.{name}.as_slice()]))"
            ));
        }
        LaicType::List(inner) => emit_list_column(name, inner, out),
        LaicType::Optional(inner) => emit_optional_column(name, inner, out),
        LaicType::Map(k, v) => emit_map_column(name, k, v, out),
    }
}

fn emit_list_column(name: &str, inner: &LaicType, out: &mut String) {
    let builder = arrow_builder_type(inner);
    let deref = if needs_deref(inner) { "*" } else { "" };

    let append = match inner {
        LaicType::String => "values.append_value(item);".to_string(),
        LaicType::Bytes | LaicType::Tensor { .. } => "values.append_value(item);".to_string(),
        LaicType::Optional(opt_inner) => {
            let inner_builder = arrow_builder_type(opt_inner);
            // Override builder for list<optional<T>>
            out.push_str(&format!(
                "{{\n            let mut builder = ListBuilder::new({inner_builder}::new());\n"
            ));
            let opt_deref = if needs_deref(opt_inner) { "*" } else { "" };
            let opt_append = if matches!(opt_inner.as_ref(), LaicType::String | LaicType::Bytes) {
                "values.append_value(v)".to_string()
            } else {
                format!("values.append_value({opt_deref}v)")
            };
            out.push_str("            let values = builder.values();\n");
            out.push_str(&format!("            for item in &self.{name} {{\n"));
            out.push_str(&format!(
                "                match item {{ Some(v) => {opt_append}, None => values.append_null() }};\n"
            ));
            out.push_str("            }\n");
            out.push_str("            builder.append(true);\n");
            out.push_str("            Arc::new(builder.finish())\n");
            out.push_str("        }");
            return;
        }
        _ => format!("values.append_value({deref}item);"),
    };

    out.push_str(&format!(
        "{{\n            let mut builder = ListBuilder::new({builder}::new());\n"
    ));
    out.push_str("            let values = builder.values();\n");
    out.push_str(&format!("            for item in &self.{name} {{\n"));
    out.push_str(&format!("                {append}\n"));
    out.push_str("            }\n");
    out.push_str("            builder.append(true);\n");
    out.push_str("            Arc::new(builder.finish())\n");
    out.push_str("        }");
}

fn emit_optional_column(name: &str, inner: &LaicType, out: &mut String) {
    match inner {
        LaicType::String => {
            out.push_str(&format!(
                "Arc::new(StringArray::from(vec![self.{name}.as_deref()]))"
            ));
        }
        LaicType::Bytes => {
            out.push_str(&format!(
                "Arc::new(BinaryArray::from(vec![self.{name}.as_deref()]))"
            ));
        }
        LaicType::Bool => {
            out.push_str(&format!("Arc::new(BooleanArray::from(vec![self.{name}]))"));
        }
        LaicType::Tensor { .. } => {
            out.push_str(&format!(
                "Arc::new(BinaryArray::from(vec![self.{name}.as_deref()]))"
            ));
        }
        LaicType::List(list_inner) => {
            let builder = arrow_builder_type(list_inner);
            let deref = if needs_deref(list_inner) { "*" } else { "" };
            let append = if matches!(list_inner.as_ref(), LaicType::String | LaicType::Bytes) {
                "values.append_value(item);".to_string()
            } else {
                format!("values.append_value({deref}item);")
            };
            out.push_str(&format!(
                "{{\n            let mut builder = ListBuilder::new({builder}::new());\n"
            ));
            out.push_str(&format!("            match &self.{name} {{\n"));
            out.push_str("                Some(list) => {\n");
            out.push_str("                    let values = builder.values();\n");
            out.push_str("                    for item in list {\n");
            out.push_str(&format!("                        {append}\n"));
            out.push_str("                    }\n");
            out.push_str("                    builder.append(true);\n");
            out.push_str("                }\n");
            out.push_str("                None => builder.append(false),\n");
            out.push_str("            }\n");
            out.push_str("            Arc::new(builder.finish())\n");
            out.push_str("        }");
        }
        _ => {
            // Numeric optional types
            let arr_type = match inner {
                LaicType::I8 => "Int8Array",
                LaicType::I16 => "Int16Array",
                LaicType::I32 => "Int32Array",
                LaicType::I64 => "Int64Array",
                LaicType::U8 => "UInt8Array",
                LaicType::F32 => "Float32Array",
                LaicType::F64 => "Float64Array",
                _ => "Int32Array",
            };
            out.push_str(&format!("Arc::new({arr_type}::from(vec![self.{name}]))"));
        }
    }
}

fn emit_map_column(name: &str, key: &LaicType, value: &LaicType, out: &mut String) {
    let key_builder = arrow_builder_type(key);
    let val_builder = arrow_builder_type(value);
    let key_append = map_append_expr(key, "k");
    let val_append = map_append_expr(value, "v");

    out.push_str(&format!(
        "{{\n            let mut builder = MapBuilder::new(None, {key_builder}::new(), {val_builder}::new());\n"
    ));
    out.push_str(&format!("            for (k, v) in &self.{name} {{\n"));
    out.push_str(&format!("                builder.keys().{key_append};\n"));
    out.push_str(&format!("                builder.values().{val_append};\n"));
    out.push_str("            }\n");
    out.push_str(
        "            builder.append(true).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;\n",
    );
    out.push_str("            Arc::new(builder.finish())\n");
    out.push_str("        }");
}

fn map_append_expr(ty: &LaicType, var: &str) -> String {
    match ty {
        LaicType::String | LaicType::Bytes => format!("append_value({var})"),
        _ => format!("append_value(*{var})"),
    }
}
