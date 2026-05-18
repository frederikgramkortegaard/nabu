use crate::error::Error;
use crate::sql::lexer::LexerContext;
use crate::sql::parser::ParserContext;
use crate::storage::Database;
use crate::storage::TableBuilder;
use crate::types::{ColumnType, QueryResult};
use std::fs;

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
            } else if input.starts_with(".exec_file ") && (*&self.active_database.is_some()) {
                let file_name = input
                    .split_once(".exec_file ")
                    .ok_or(Error::UnexpectedEof {
                        expected: "file path".into(),
                    })?
                    .1;

                let script = fs::read_to_string(file_name)?;
                let scripts: Vec<&str> = script.split(';').collect();
                let Some(db) = &self.active_database else {
                    unreachable!();
                };
                for s in scripts {
                    let clean = s.trim();
                    if clean.is_empty() {
                        continue;
                    }
                    println!("/.. {:?}", clean);

                    let result = run_query(db, clean)?;
                    println!("{:?}\n", result);
                }
            } else if input.starts_with(".create_table ") {
                // .create_table name col1:type1 col2:type2 ...
                // types: number, bool, varchar(N)
                let Some(db) = &mut self.active_database else {
                    println!("No database loaded\n");
                    continue;
                };

                let args = input.strip_prefix(".create_table ").unwrap();
                let mut parts = args.split_whitespace();

                let Some(table_name) = parts.next() else {
                    println!("Usage: .create_table name col1:type1 col2:type2 ...\n");
                    continue;
                };

                let mut columns: Vec<(String, ColumnType)> = Vec::new();
                let mut parse_error = false;

                for part in parts {
                    let Some((col_name, type_str)) = part.split_once(':') else {
                        println!("Invalid column format '{}', expected name:type\n", part);
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
                            println!("Invalid varchar length: {}\n", len_str);
                            parse_error = true;
                            break;
                        };
                        ColumnType::Varchar(len)
                    } else {
                        println!(
                            "Unknown type '{}', use number, bool, or varchar(N)\n,",
                            type_str
                        );
                        parse_error = true;
                        break;
                    };

                    columns.push((col_name.to_string(), col_type));
                }

                if parse_error {
                    continue;
                }

                if columns.is_empty() {
                    println!("No columns specified\n");
                    continue;
                }

                let mut builder = TableBuilder::new(table_name);
                for (col_name, col_type) in columns {
                    builder = builder.column(col_name, col_type);
                }

                match db.create_table(builder) {
                    Ok(_) => println!("Created table '{}'\n", table_name),
                    Err(e) => println!("Error: {:?}\n", e),
                }
            } else if let Some(db) = &self.active_database {
                let result = run_query(db, input);
                println!("{:?}\n", result);
            } else {
                println!("use .load_database <path> to load a database before running a query\n")
            }
        }

        Ok(())
    }
}
