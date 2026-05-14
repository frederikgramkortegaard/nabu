use crate::sql::{LexerContext, ParserContext, TypecheckerContext};
use crate::storage::Database;
use std::io::stdin;

#[derive(Debug)]
pub struct Repl<'a> {
    pub history: Vec<String>,
    pub database: &'a Database<'a>,
}

impl<'a> Repl<'a> {
    pub fn start(&mut self) -> Result<(), &str> {
        let mut input = String::new();

        while stdin().read_line(&mut input).is_ok() {
            input = input.trim().into();
            self.history.push(input.clone());

            // Lex the input
            let tokens = match LexerContext::lex(&input) {
                Ok(tokens) => tokens,
                Err(e) => {
                    eprintln!(
                        "Lexing error at line {}, column {}: {}",
                        e.row, e.column, e.message
                    );

                    input.clear();
                    continue;
                }
            };
            println!("{:?}", tokens);

            let stmt = match ParserContext::parse(&tokens) {
                Ok(stmt) => stmt,
                Err(e) => {
                    eprintln!("{}", e.message);

                    input.clear();
                    continue;
                }
            };

            if let Err(e) = TypecheckerContext::typecheck(&stmt, self.database) {
                eprintln!("{}", e.message);
                input.clear();
                continue;
            }

            println!("Parsed Statement: {:?}", stmt);
        }

        Ok(())
    }
}
