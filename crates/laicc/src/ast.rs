//! AST types for the LAIC IDL.

/// A complete `.laic` file.
#[derive(Debug, Clone)]
pub struct LaicFile {
    /// File-level version declaration (e.g., "1.0.0").
    pub version: String,
    /// Skill definitions in declaration order.
    pub skills: Vec<SkillDef>,
}

/// A single `skill { ... }` block.
#[derive(Debug, Clone)]
pub struct SkillDef {
    /// Skill name (identifier used in code generation).
    pub name: String,
    /// Wire protocol skill ID (the `id = "..."` value).
    pub id: String,
    /// Input struct definition.
    pub input: StructDef,
    /// Output struct definition.
    pub output: StructDef,
    /// Optional error variants.
    pub errors: Vec<ErrorVariant>,
}

/// A struct definition (used for input and output).
#[derive(Debug, Clone)]
pub struct StructDef {
    /// Struct name (e.g., `EmbeddingInput`).
    pub name: String,
    /// Fields in declaration order.
    pub fields: Vec<FieldDef>,
}

/// A single field in a struct.
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: LaicType,
    /// Optional default value.
    pub default: Option<Literal>,
}

/// Types supported by the LAIC IDL.
#[derive(Debug, Clone, PartialEq)]
pub enum LaicType {
    /// UTF-8 string.
    String,
    /// Raw byte buffer.
    Bytes,
    /// Boolean.
    Bool,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 8-bit integer.
    U8,
    /// 32-bit float (IEEE 754).
    F32,
    /// 64-bit float (IEEE 754).
    F64,
    /// Tensor with element dtype and shape dimensions.
    Tensor {
        /// Element data type.
        dtype: TensorElementType,
        /// Shape dimensions.
        dims: Vec<Dimension>,
    },
    /// List of elements: `list<T>`.
    List(Box<LaicType>),
    /// Optional value: `optional<T>`.
    Optional(Box<LaicType>),
    /// Map from key to value: `map<K, V>`.
    Map(Box<LaicType>, Box<LaicType>),
}

/// Tensor element types (subset of scalar types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorElementType {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 8-bit integer.
    U8,
    /// Boolean.
    Bool,
}

impl TensorElementType {
    /// Return the IDL source representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::Bool => "bool",
        }
    }
}

/// A single dimension in a tensor shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    /// Fixed size (e.g., `768`).
    Fixed(usize),
    /// Dynamic/wildcard (`_` or a named dim like `batch`).
    Dynamic(Option<String>),
}

/// A named error variant in an `error { ... }` block.
#[derive(Debug, Clone)]
pub struct ErrorVariant {
    /// Variant name (e.g., `INPUT_TOO_LONG`).
    pub name: String,
    /// Numeric error code (positive integer).
    pub code: u16,
}

/// Literal values for field defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// String literal.
    String(String),
    /// Integer literal.
    Integer(i64),
    /// Float literal.
    Float(f64),
    /// Boolean literal.
    Bool(bool),
}
