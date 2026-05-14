mod repl;
mod sql;
mod storage;

use storage::{ColumnType, Database, Table};

fn main() {
    let mut mydb = Database::new();

    let table = Table::new(
        "MyTable".to_string(),
        [
            ("id".to_string(), ColumnType::Number),
            ("age".to_string(), ColumnType::Number),
            ("username".to_string(), ColumnType::Varchar(32)),
            ("email".to_string(), ColumnType::Varchar(256)),
        ],
    );

    let _ = mydb.add_table(&table);

    println!("{:?}", table);

    let mut repl = repl::Repl {
        history: vec![],
        database: &mydb,
    };
    let res = repl.start();
    println!("{:?}", res);
}
