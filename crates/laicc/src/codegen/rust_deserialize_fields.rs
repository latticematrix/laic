//! Helper functions for `from_arrow_ipc()` field extraction.

use crate::ast::{Dimension, LaicType, TensorElementType};
use crate::codegen::rust_types::arrow_array_type;

pub(crate) fn primitive_value_expr(ty: &LaicType) -> &'static str {
    match ty {
        LaicType::String => ".value(0).to_string()",
        LaicType::Bool => ".value(0)",
        LaicType::I8 => ".value(0)",
        LaicType::I16 => ".value(0)",
        LaicType::I32 => ".value(0)",
        LaicType::I64 => ".value(0)",
        LaicType::U8 => ".value(0)",
        LaicType::F32 => ".value(0)",
        LaicType::F64 => ".value(0)",
        _ => ".value(0)",
    }
}

pub(crate) fn emit_tensor_extraction(
    out: &mut String,
    name: &str,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    out.push_str(&format!("        let {name} = {{\n"));
    emit_tensor_metadata_validation(out, name, dtype, dims);

    // Extract data
    out.push_str(&format!(
        "            let col = batch.column_by_name(\"{name}\")\n"
    ));
    out.push_str("                .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())\n");
    out.push_str(&format!(
        "                .ok_or(\"missing '{}' BinaryArray\")?;\n",
        name
    ));
    out.push_str("            col.value(0).to_vec()\n");
    out.push_str("        };\n");
}

fn emit_tensor_metadata_validation(
    out: &mut String,
    name: &str,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    // WHY: tensor metadata is part of the schema-field contract, even when the
    // tensor bytes live inside list<...> or optional<...>. Validating it here keeps
    // Rust aligned with Python/TypeScript and prevents silent cross-language drift.
    out.push_str("            let schema = batch.schema();\n");
    out.push_str(&format!(
        "            let field = schema.field_with_name(\"{name}\")\n"
    ));
    out.push_str(&format!(
        "                .map_err(|_| \"missing '{}' field\".to_string())?;\n",
        name
    ));
    out.push_str("            let meta = field.metadata();\n");

    emit_tensor_dtype_validation(out, name, dtype);
    emit_tensor_shape_validation(out, name, dims);
}

fn emit_tensor_dtype_validation(out: &mut String, name: &str, dtype: &TensorElementType) {
    out.push_str("            let dtype_str = meta.get(\"laic.tensor.dtype\")\n");
    out.push_str(&format!(
        "                .ok_or(\"field '{}': missing tensor dtype metadata\")?;\n",
        name
    ));
    out.push_str(&format!(
        "            if dtype_str != \"{}\" {{\n",
        dtype.as_str()
    ));
    out.push_str(&format!(
        "                return Err(format!(\"field '{}': expected dtype {}, got {{}}\", dtype_str).into());\n",
        name,
        dtype.as_str()
    ));
    out.push_str("            }\n");
}

fn emit_tensor_shape_validation(out: &mut String, name: &str, dims: &[Dimension]) {
    out.push_str("            let shape_str = meta.get(\"laic.tensor.shape\")\n");
    out.push_str(&format!(
        "                .ok_or(\"field '{}': missing tensor shape metadata\")?;\n",
        name
    ));
    // Parse shape
    out.push_str(
        "            let shape: Vec<usize> = shape_str.trim_matches(|c| c == '[' || c == ']')\n",
    );
    out.push_str("                .split(',')\n");
    out.push_str("                .filter(|s| !s.is_empty())\n");
    out.push_str("                .map(|s| s.trim().parse::<usize>())\n");
    out.push_str("                .collect::<Result<_, _>>()?;\n");

    // Validate ndim
    out.push_str(&format!(
        "            if shape.len() != {} {{\n",
        dims.len()
    ));
    out.push_str(&format!(
        "                return Err(format!(\"field '{}': expected {} dimensions, got {{}}\", shape.len()).into());\n",
        name,
        dims.len()
    ));
    out.push_str("            }\n");

    // Validate fixed dims
    for (i, dim) in dims.iter().enumerate() {
        if let Dimension::Fixed(size) = dim {
            out.push_str(&format!("            if shape[{i}] != {size} {{\n"));
            out.push_str(&format!(
                "                return Err(format!(\"field '{}': dim {} expected {}, got {{}}\", shape[{i}]).into());\n",
                name, i, size
            ));
            out.push_str("            }\n");
        }
    }
}

pub(crate) fn emit_list_extraction(out: &mut String, name: &str, inner: &LaicType) {
    out.push_str(&format!("        let {name} = {{\n"));
    out.push_str(&format!(
        "            let list_arr = batch.column_by_name(\"{name}\")\n"
    ));
    out.push_str("                .and_then(|c| c.as_any().downcast_ref::<ListArray>())\n");
    out.push_str(&format!(
        "                .ok_or(\"missing '{}' ListArray\")?;\n",
        name
    ));
    out.push_str("            let inner_arr = list_arr.value(0);\n");

    match inner {
        LaicType::String => {
            out.push_str(
                "            let typed = inner_arr.as_any().downcast_ref::<StringArray>()\n",
            );
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected StringArray\")?;\n",
                name
            ));
            out.push_str(
                "            (0..typed.len()).map(|i| typed.value(i).to_string()).collect::<Vec<_>>()\n",
            );
        }
        LaicType::Bytes => {
            out.push_str(
                "            let typed = inner_arr.as_any().downcast_ref::<BinaryArray>()\n",
            );
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected BinaryArray\")?;\n",
                name
            ));
            out.push_str(
                "            (0..typed.len()).map(|i| typed.value(i).to_vec()).collect::<Vec<_>>()\n",
            );
        }
        LaicType::Bool => {
            out.push_str(
                "            let typed = inner_arr.as_any().downcast_ref::<BooleanArray>()\n",
            );
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected BooleanArray\")?;\n",
                name
            ));
            out.push_str(
                "            (0..typed.len()).map(|i| typed.value(i)).collect::<Vec<_>>()\n",
            );
        }
        LaicType::Optional(opt_inner) => {
            let arr_type = arrow_array_type(opt_inner);
            let val_expr = list_inner_value_expr(opt_inner);
            out.push_str(&format!(
                "            let typed = inner_arr.as_any().downcast_ref::<{arr_type}>()\n"
            ));
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected {arr_type}\")?;\n",
                name
            ));
            out.push_str(&format!(
                "            (0..typed.len()).map(|i| if typed.is_null(i) {{ None }} else {{ Some({val_expr}) }}).collect::<Vec<_>>()\n"
            ));
        }
        LaicType::Tensor { .. } => {
            if let LaicType::Tensor { dtype, dims } = inner {
                emit_tensor_metadata_validation(out, name, dtype, dims);
            }
            out.push_str(
                "            let typed = inner_arr.as_any().downcast_ref::<BinaryArray>()\n",
            );
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected BinaryArray\")?;\n",
                name
            ));
            out.push_str(
                "            (0..typed.len()).map(|i| typed.value(i).to_vec()).collect::<Vec<_>>()\n",
            );
        }
        _ => {
            let arr_type = arrow_array_type(inner);
            let val_expr = list_inner_value_expr(inner);
            out.push_str(&format!(
                "            let typed = inner_arr.as_any().downcast_ref::<{arr_type}>()\n"
            ));
            out.push_str(&format!(
                "                .ok_or(\"field '{}': expected {arr_type}\")?;\n",
                name
            ));
            out.push_str(&format!(
                "            (0..typed.len()).map(|i| {val_expr}).collect::<Vec<_>>()\n"
            ));
        }
    }
    out.push_str("        };\n");
}

fn list_inner_value_expr(ty: &LaicType) -> String {
    match ty {
        LaicType::String => "typed.value(i).to_string()".into(),
        LaicType::Bytes => "typed.value(i).to_vec()".into(),
        _ => "typed.value(i)".into(),
    }
}

pub(crate) fn emit_optional_extraction(out: &mut String, name: &str, inner: &LaicType) {
    out.push_str(&format!("        let {name} = {{\n"));
    out.push_str(&format!(
        "            let col = batch.column_by_name(\"{name}\")\n"
    ));
    out.push_str(&format!(
        "                .ok_or(\"missing '{}' column\")?;\n",
        name
    ));
    if let LaicType::Tensor { dtype, dims } = inner {
        emit_tensor_metadata_validation(out, name, dtype, dims);
    }
    out.push_str("            if col.is_null(0) {\n");
    out.push_str("                None\n");
    out.push_str("            } else {\n");

    match inner {
        LaicType::String => {
            out.push_str("                Some(col.as_any().downcast_ref::<StringArray>()\n");
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected StringArray\")?\n",
                name
            ));
            out.push_str("                    .value(0).to_string())\n");
        }
        LaicType::Bytes => {
            out.push_str("                Some(col.as_any().downcast_ref::<BinaryArray>()\n");
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected BinaryArray\")?\n",
                name
            ));
            out.push_str("                    .value(0).to_vec())\n");
        }
        LaicType::Bool => {
            out.push_str("                Some(col.as_any().downcast_ref::<BooleanArray>()\n");
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected BooleanArray\")?\n",
                name
            ));
            out.push_str("                    .value(0))\n");
        }
        LaicType::Tensor { .. } => {
            out.push_str("                Some(col.as_any().downcast_ref::<BinaryArray>()\n");
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected BinaryArray\")?\n",
                name
            ));
            out.push_str("                    .value(0).to_vec())\n");
        }
        LaicType::List(list_inner) => {
            let arr_type = arrow_array_type(list_inner);
            let val_expr = list_inner_value_expr(list_inner);
            out.push_str(
                "                let list_arr = col.as_any().downcast_ref::<ListArray>()\n",
            );
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected ListArray\")?;\n",
                name
            ));
            out.push_str("                let inner_arr = list_arr.value(0);\n");
            out.push_str(&format!(
                "                let typed = inner_arr.as_any().downcast_ref::<{arr_type}>()\n"
            ));
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected {arr_type}\")?;\n",
                name
            ));
            out.push_str(&format!(
                "                Some((0..typed.len()).map(|i| {val_expr}).collect::<Vec<_>>())\n"
            ));
        }
        _ => {
            let arr_type = arrow_array_type(inner);
            let val_expr = primitive_value_expr(inner);
            out.push_str(&format!(
                "                Some(col.as_any().downcast_ref::<{arr_type}>()\n"
            ));
            out.push_str(&format!(
                "                    .ok_or(\"field '{}': expected {arr_type}\")?\n",
                name
            ));
            out.push_str(&format!("                    {val_expr})\n"));
        }
    }

    out.push_str("            }\n");
    out.push_str("        };\n");
}

pub(crate) fn emit_map_extraction(out: &mut String, name: &str, key: &LaicType, value: &LaicType) {
    let key_arr_type = arrow_array_type(key);
    let val_arr_type = arrow_array_type(value);
    let key_expr = map_value_expr(key, "key_arr");
    let val_expr = map_value_expr(value, "val_arr");

    out.push_str(&format!("        let {name} = {{\n"));
    out.push_str(&format!(
        "            let map_arr = batch.column_by_name(\"{name}\")\n"
    ));
    out.push_str("                .and_then(|c| c.as_any().downcast_ref::<MapArray>())\n");
    out.push_str(&format!(
        "                .ok_or(\"missing '{}' MapArray\")?;\n",
        name
    ));
    out.push_str("            let entries = map_arr.value(0);\n");
    out.push_str("            let struct_arr = entries.as_any().downcast_ref::<StructArray>()\n");
    out.push_str(&format!(
        "                .ok_or(\"field '{}': expected StructArray\")?;\n",
        name
    ));
    out.push_str(&format!(
        "            let key_arr = struct_arr.column(0).as_any().downcast_ref::<{key_arr_type}>()\n"
    ));
    out.push_str(&format!(
        "                .ok_or(\"field '{}': expected {key_arr_type} for keys\")?;\n",
        name
    ));
    out.push_str(&format!(
        "            let val_arr = struct_arr.column(1).as_any().downcast_ref::<{val_arr_type}>()\n"
    ));
    out.push_str(&format!(
        "                .ok_or(\"field '{}': expected {val_arr_type} for values\")?;\n",
        name
    ));
    out.push_str("            let mut map = HashMap::new();\n");
    out.push_str("            for i in 0..key_arr.len() {\n");
    out.push_str(&format!("                let k = {key_expr};\n"));
    out.push_str(&format!("                let v = {val_expr};\n"));
    out.push_str("                map.insert(k, v);\n");
    out.push_str("            }\n");
    out.push_str("            map\n");
    out.push_str("        };\n");
}

fn map_value_expr(ty: &LaicType, arr_var: &str) -> String {
    match ty {
        LaicType::String => format!("{arr_var}.value(i).to_string()"),
        LaicType::Bytes => format!("{arr_var}.value(i).to_vec()"),
        _ => format!("{arr_var}.value(i)"),
    }
}
