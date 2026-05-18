use crate::error::Error;
use crate::sql::lexer::LexerContext;
use crate::sql::parser::ParserContext;
use crate::storage::Database;
use crate::storage::TableBuilder;
use crate::types::{ColumnType, QueryResult};

use crate::analyzer;
use crate::core;
use std::io;
#[derive(Debug)]
pub struct Repl {
    pub active_database: Option<Database>,
}
fn run_query(db: &Database, query: &str) -> Result<QueryResult, Error> {
    let tokens = LexerContext::lex(query)?;
    let ast = ParserContext::parse(&tokens)?;
    let bound = analyzer::bound::bind(ast, db)?;
    analyzer::typechecker::typecheck(&bound)?;
    let result = core::engine::execute(&bound)?;
    Ok(result)
}

impl Repl {
    pub fn new() -> Self {
        Self {
            active_database: None,
        }
    }
    pub fn start(&mut self) -> Result<(), Error> {
        println!("{:?}\n", std::env::current_dir());
        loop {
            let mut input = String::new();
            let bytes_read = io::stdin().read_line(&mut input)?;

            // Ctrl-D / EOF
            if bytes_read == 0 {
                break;
            }

            let input = input.trim_end();

            if input == ".exit" {
                break;
            } else if input.starts_with(".load_database ") {
                let file_name = input
                    .split_once(".load_database ")
                    .ok_or(Error::UnexpectedEof {
                        expected: "file path".into(),
                    })?
                    .1;

                self.active_database = Some(Database::new(file_name)?);
                continue;
            } else if input.starts_with(".create_table ") {
                // .create_table name col1:type1 col2:type2 ...
                // types: number, bool, varchar(N)
                let Some(db) = &mut self.active_database else {
                    println!("No database loaded");
                    continue;
                };

                let args = input.strip_prefix(".create_table ").unwrap();
                let mut parts = args.split_whitespace();

                let Some(table_name) = parts.next() else {
                    println!("Usage: .create_table name col1:type1 col2:type2 ...");
                    continue;
                };

                let mut columns: Vec<(String, ColumnType)> = Vec::new();
                let mut parse_error = false;

                for part in parts {
                    let Some((col_name, type_str)) = part.split_once(':') else {
                        println!("Invalid column format '{}', expected name:type", part);
                        parse_error = true;
                        break;
                    };

                    let col_type = if type_str == "number" {
                        ColumnType::Number
                    } else if type_str == "bool" {
                        ColumnType::Bool
                    } else if type_str.starts_with("varchar(") && type_str.ends_with(')') {
                        let len_str = &type_str[8..type_str.len() - 1];
                        let Ok(len) = len_str.parse::<usize>() else {
                            println!("Invalid varchar length: {}", len_str);
                            parse_error = true;
                            break;
                        };
                        ColumnType::Varchar(len)
                    } else {
                        println!("Unknown type '{}', use number, bool, or varchar(N)", type_str);
                        parse_error = true;
                        break;
                    };

                    columns.push((col_name.to_string(), col_type));
                }

                if parse_error {
                    continue;
                }

                if columns.is_empty() {
                    println!("No columns specified");
                    continue;
                }

                let mut builder = TableBuilder::new(table_name);
                for (col_name, col_type) in columns {
                    builder = builder.column(col_name, col_type);
                }

                match db.create_table(builder) {
                    Ok(_) => println!("Created table '{}'", table_name),
                    Err(e) => println!("Error: {:?}", e),
                }
            } else if let Some(db) = &self.active_database {
                let result = run_query(db, input);
                println!("{:?}", result);
            } else {
                println!("use .load_database <path> to load a database before running a query")
            }
        }

        Ok(())
    }
}
