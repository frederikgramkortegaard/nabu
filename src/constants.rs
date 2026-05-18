/// Maximum length for table names
pub const MAX_TABLE_NAME_LEN: usize = 64;

/// Maximum length for serialized columns string in table metadata
/// @TODO:  In the future we should store this some other way.
pub const MAX_COLUMNS_STR_LEN: usize = 1024;

/// Size of a Number column (f64)
pub const NUMBER_SIZE: usize = 8;

/// Size of a Bool column
pub const BOOL_SIZE: usize = 1;

/// Maximum length for a Varchar column
pub const MAX_VARCHAR_LEN: usize = 65535;

/// Maximum number of user columns per table
pub const MAX_COLUMNS: usize = 64;

/// Size of null bitmap (MAX_COLUMNS / 8)
pub const NULL_BITMAP_SIZE: usize = 8;
