//! Generate Python `from_ipc()` deserialization methods.

use crate::ast::{Dimension, FieldDef, LaicType, StructDef, TensorElementType};
use crate::codegen::python_bytes_literal;
use crate::codegen::python_types::pyarrow_type;

/// Emit `from_ipc(cls, data: bytes) -> Self` classmethod body.
pub fn generate_from_ipc(
    out: &mut String,
    def: &StructDef,
    skill_id: &str,
    version: &str,
    direction: &str,
) {
    out.push_str("    @classmethod\n");
    out.push_str(&format!(
        "    def from_ipc(cls, data: bytes) -> {}:\n",
        def.name
    ));
    out.push_str("        \"\"\"Deserialize from Arrow IPC stream format.\"\"\"\n");
    out.push_str("        reader = ipc.open_stream(data)\n");
    out.push_str("        batch = reader.read_next_batch()\n");

    // Cardinality: exactly 1 row
    out.push_str("        if batch.num_rows == 0:\n");
    out.push_str(
        "            raise ValueError(\"cardinality error: RecordBatch has 0 rows, expected 1\")\n",
    );
    out.push_str("        if batch.num_rows > 1:\n");
    out.push_str("            raise ValueError(\n");
    out.push_str("                f\"cardinality error: RecordBatch has {batch.num_rows} rows, expected 1\"\n");
    out.push_str("            )\n");

    // Trailing batch check
    out.push_str("        try:\n");
    out.push_str("            reader.read_next_batch()\n");
    out.push_str("            raise ValueError(\"cardinality error: stream contains more than one RecordBatch\")\n");
    out.push_str("        except StopIteration:\n");
    out.push_str("            pass\n");

    // Metadata validation
    emit_metadata_validation(out, skill_id, version, direction);

    // Extract fields
    for field in &def.fields {
        emit_field_extraction(out, field);
    }

    // Build return
    out.push_str("        return cls(\n");
    for field in &def.fields {
        out.push_str(&format!("            {}={},\n", field.name, field.name));
    }
    out.push_str("        )\n");
    out.push('\n');
}

fn emit_metadata_validation(out: &mut String, skill_id: &str, version: &str, direction: &str) {
    out.push_str("        meta = batch.schema.metadata or {}\n");
    for (key, expected) in [
        ("laic.skill_id", skill_id),
        ("laic.version", version),
        ("laic.direction", direction),
    ] {
        out.push_str(&format!("        _v = meta.get(b\"{key}\")\n"));
        out.push_str("        if _v is None:\n");
        out.push_str(&format!(
            "            raise ValueError(\"missing required metadata key '{key}'\")\n"
        ));
        out.push_str(&format!(
            "        if _v != {}:\n",
            python_bytes_literal(expected)
        ));
        out.push_str(&format!(
            "            raise ValueError(f\"metadata '{key}' mismatch: got '{{_v.decode()}}'\")\n"
        ));
    }
}

fn emit_field_extraction(out: &mut String, field: &FieldDef) {
    let name = &field.name;
    match &field.ty {
        LaicType::Tensor { dtype, dims } => {
            emit_tensor_extraction(out, name, dtype, dims);
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
            emit_scalar_extraction(out, name, &field.ty, &field.default);
        }
    }
}

fn emit_scalar_extraction(
    out: &mut String,
    name: &str,
    ty: &LaicType,
    default: &Option<crate::ast::Literal>,
) {
    let expected_type = pyarrow_type(ty);
    match default {
        Some(lit) => {
            let default_val = crate::codegen::python_types::literal_to_python(lit);
            out.push_str(&format!(
                "        _col = batch.column(\"{name}\") if \"{name}\" in batch.schema.names else None\n"
            ));
            out.push_str("        if _col is not None:\n");
            out.push_str(&format!(
                "            _field = batch.schema.field(\"{name}\")\n"
            ));
            out.push_str(&format!("            if _field.type != {expected_type}:\n"));
            out.push_str(&format!(
                "                raise ValueError(f\"field '{name}': expected {expected_type}, got {{_field.type}}\")\n"
            ));
            out.push_str(&format!("            {name} = _col[0].as_py()\n"));
            out.push_str("        else:\n");
            out.push_str(&format!("            {name} = {default_val}\n"));
        }
        None => {
            // WHY: .as_py() is the universal PyArrow scalar to Python conversion.
            out.push_str(&format!("        _col = batch.column(\"{name}\")\n"));
            out.push_str(&format!(
                "        _field = batch.schema.field(\"{name}\")\n"
            ));
            out.push_str(&format!("        if _field.type != {expected_type}:\n"));
            out.push_str(&format!(
                "            raise ValueError(f\"field '{name}': expected {expected_type}, got {{_field.type}}\")\n"
            ));
            out.push_str(&format!("        {name} = _col[0].as_py()\n"));
        }
    }
}

fn emit_tensor_extraction(
    out: &mut String,
    name: &str,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    out.push_str(&format!(
        "        {name} = batch.column(\"{name}\")[0].as_py()\n"
    ));
    emit_tensor_metadata_validation(out, name, dtype, dims);
}

fn emit_list_extraction(out: &mut String, name: &str, inner: &LaicType) {
    match inner {
        LaicType::Tensor { dtype, dims } => {
            // WHY: list<tensor> keeps tensor metadata on the outer field. Validate the
            // field contract before materializing the list so Python stays aligned with
            // the stronger TypeScript path and rejects drifted IPC early.
            emit_tensor_metadata_validation(out, name, dtype, dims);
            out.push_str(&format!(
                "        {name} = [v.as_py() for v in batch.column(\"{name}\")[0].values]\n"
            ));
        }
        _ => {
            // WHY: .as_py() on a ListScalar returns a native Python list.
            out.push_str(&format!(
                "        {name} = batch.column(\"{name}\")[0].as_py()\n"
            ));
        }
    }
}

fn emit_optional_extraction(out: &mut String, name: &str, inner: &LaicType) {
    match inner {
        LaicType::Tensor { dtype, dims } => {
            // WHY: nullable tensor fields still publish mandatory field metadata. The
            // contract is attached to the schema field, not to whether row 0 happens to
            // be null, so validate before checking row validity.
            emit_tensor_metadata_validation(out, name, dtype, dims);
            out.push_str(&format!("        _val = batch.column(\"{name}\")[0]\n"));
            out.push_str(&format!(
                "        {name} = None if not _val.is_valid else _val.as_py()\n"
            ));
        }
        _ => {
            out.push_str(&format!("        _val = batch.column(\"{name}\")[0]\n"));
            out.push_str(&format!(
                "        {name} = None if not _val.is_valid else _val.as_py()\n"
            ));
        }
    }
}

fn emit_map_extraction(out: &mut String, name: &str, _k: &LaicType, _v: &LaicType) {
    // WHY: MapScalar.as_py() returns list of tuples [(k, v), ...]; convert to dict.
    out.push_str(&format!(
        "        _map_val = batch.column(\"{name}\")[0].as_py()\n"
    ));
    out.push_str(&format!(
        "        {name} = dict(_map_val) if _map_val is not None else {{}}\n"
    ));
}

fn emit_tensor_metadata_validation(
    out: &mut String,
    name: &str,
    dtype: &TensorElementType,
    dims: &[Dimension],
) {
    out.push_str(&format!(
        "        _field = batch.schema.field(\"{name}\")\n"
    ));
    out.push_str("        _fmeta = _field.metadata or {}\n");

    out.push_str("        _dtype = _fmeta.get(b\"laic.tensor.dtype\")\n");
    out.push_str(&format!(
        "        if _dtype is None:\n            raise ValueError(\"field '{name}': missing tensor dtype metadata\")\n"
    ));
    out.push_str(&format!("        if _dtype != b\"{}\":\n", dtype.as_str()));
    out.push_str(&format!(
        "            raise ValueError(f\"field '{name}': expected dtype {}, got {{_dtype.decode()}}\")\n",
        dtype.as_str()
    ));

    out.push_str("        _shape_str = _fmeta.get(b\"laic.tensor.shape\")\n");
    out.push_str(&format!(
        "        if _shape_str is None:\n            raise ValueError(\"field '{name}': missing tensor shape metadata\")\n"
    ));
    out.push_str(
        "        _shape = [int(x) for x in _shape_str.decode().strip(\"[]\").split(\",\") if x]\n",
    );
    out.push_str(&format!(
        "        if len(_shape) != {ndim}:\n",
        ndim = dims.len()
    ));
    out.push_str(&format!(
        "            raise ValueError(f\"field '{name}': expected {ndim} dimensions, got {{len(_shape)}}\")\n",
        ndim = dims.len()
    ));

    for (i, dim) in dims.iter().enumerate() {
        if let Dimension::Fixed(size) = dim {
            out.push_str(&format!("        if _shape[{i}] != {size}:\n"));
            out.push_str(&format!(
                "            raise ValueError(f\"field '{name}': dim {i} expected {size}, got {{_shape[{i}]}}\")\n"
            ));
        }
    }
}
