#[derive(Debug)]
pub enum Statement {
    Insert {
        values: Vec<Value>,
        table_name: String,
    },
    Select,
}

#[derive(Debug)]
pub enum Value {
    Number(f64),
    Varchar(String),
}
