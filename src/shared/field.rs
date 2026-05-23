use arrow::datatypes::{DataType, Field};
use std::collections::HashMap;

/// Extension trait for Field - adds storage-specific functionality
pub trait FieldExt {
    fn byte_size(&self) -> usize;
}

impl FieldExt for Field {
    fn byte_size(&self) -> usize {
        self.metadata()
            .get("size")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| default_size_for_type(self.data_type()))
    }
}

/// Default byte size for a DataType (used if not specified in metadata)
pub fn default_size_for_type(dt: &DataType) -> usize {
    match dt {
        DataType::Boolean => 1,
        DataType::Float64 => 8,
        DataType::Int64 => 8,
        DataType::FixedSizeBinary(n) => *n as usize,
        _ => 0, // Variable-length types need explicit size in metadata
    }
}

/// Create an owned Field with storage size metadata
pub fn field_with_size(name: &str, data_type: DataType, size: usize) -> Field {
    let metadata = HashMap::from([("size".to_string(), size.to_string())]);
    Field::new(name, data_type, false).with_metadata(metadata)
}

/// Serialize a DataType to a compact string for storage
pub fn serialize_data_type(dt: &DataType) -> String {
    match dt {
        DataType::Float64 => "f64".to_string(),
        DataType::Boolean => "bool".to_string(),
        DataType::Utf8 => "utf8".to_string(),
        DataType::Binary => "bin".to_string(),
        DataType::FixedSizeBinary(n) => format!("fsb{}", n),
        _ => format!("{:?}", dt),
    }
}

/// Deserialize a DataType from storage string
pub fn deserialize_data_type(s: &str) -> Result<DataType, String> {
    match s {
        "f64" => Ok(DataType::Float64),
        "bool" => Ok(DataType::Boolean),
        "utf8" => Ok(DataType::Utf8),
        "bin" => Ok(DataType::Binary),
        s if s.starts_with("fsb") => s[3..]
            .parse::<i32>()
            .map(DataType::FixedSizeBinary)
            .map_err(|_| format!("invalid fixed size binary: {}", s)),
        _ => Err(format!("unknown data type: {}", s)),
    }
}

/// Serialize a Field to storage format "name:type:size"
pub fn serialize_field(field: &Field) -> String {
    format!(
        "{}:{}:{}",
        field.name(),
        serialize_data_type(field.data_type()),
        field.byte_size()
    )
}

/// Parse a Field from storage format "name:type:size"
pub fn parse_field(s: &str) -> Result<Field, String> {
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Err(format!("invalid field format '{}', expected name:type:size", s));
    }
    let name = parts[0];
    let data_type = deserialize_data_type(parts[1])?;
    let size = parts[2]
        .parse::<usize>()
        .map_err(|_| format!("invalid size '{}' in field '{}'", parts[2], s))?;
    Ok(field_with_size(name, data_type, size))
}
