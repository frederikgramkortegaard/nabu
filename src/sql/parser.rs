use super::ast::{Statement, Value};
use super::lexer::{Token, TokenType};

/// Error type returned when parsing fails.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

/// The parser context that maintains state during parsing.
#[derive(Debug, Clone)]
pub struct ParserContext<'a> {
    tokens: &'a Vec<Token>,
    position: usize,
}

impl<'a> ParserContext<'a> {
    fn peek(&self) -> Option<&Token> {
        self.peek_offset(0)
    }

    fn peek_offset(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.position + offset)
    }

    fn consume(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position)?.clone();
        self.position += 1;
        Some(token)
    }

    fn consume_optional(&mut self, expected_type: TokenType) -> Option<Token> {
        match self.peek() {
            Some(token) if token.tag == expected_type => self.consume(),
            _ => None,
        }
    }

    fn consume_assert(
        &mut self,
        expected_type: TokenType,
        message: String,
    ) -> Result<Token, ParseError> {
        match self.consume() {
            Some(tok) if tok.tag == expected_type => Ok(tok),
            Some(tok) => Err(ParseError {
                message: format!(
                    "{} at {}:{} (got {:?})",
                    message, tok.row, tok.column, tok.tag
                ),
            }),
            None => Err(ParseError {
                message: format!("{} (unexpected end of input)", message),
            }),
        }
    }

    fn parse_primary(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(token) => match token.tag {
                // Parenthesized expression
                TokenType::LParen => {
                    self.consume(); // consume '('
                    let val = self.parse_primary()?;
                    self.consume_assert(TokenType::RParen, "Expected ')' after value".to_string())?;
                    Ok(val)
                }

                // Number literal
                TokenType::Number => {
                    let token = self.consume().expect("token verified by peek");
                    let value = token.lexeme.parse::<f64>().map_err(|_| ParseError {
                        message: format!("Failed to parse number: {}", token.lexeme),
                    })?;
                    Ok(Value::Number(value))
                }

                TokenType::String => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Varchar(val.lexeme))
                }

                TokenType::Identifier => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Varchar(val.lexeme))
                }

                _ => Err(ParseError {
                    message: format!("Unexpected token in expression: {:?}", token.tag),
                }),
            },
            None => Err(ParseError {
                message: "Unexpected end of input in expression".to_string(),
            }),
        }
    }

    fn parse_values(&mut self) -> Result<Vec<Value>, ParseError> {
        self.consume_optional(TokenType::LParen);

        let mut vals: Vec<Value> = vec![];

        while let Ok(val) = self.parse_primary() {
            vals.push(val);
            self.consume_optional(TokenType::Comma);
        }

        self.consume_optional(TokenType::RParen);
        Ok(vals)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(token) => match token.tag {
                TokenType::Insert => {
                    println!("{:?}", self.peek());
                    self.consume();
                    println!("{:?}", self.peek());
                    let values = self.parse_values()?;
                    println!("{:?}", self.peek());
                    self.consume_assert(
                        TokenType::Into,
                        "Expected 'INTO' after 'VALUES' during 'INSERT' statement".to_string(),
                    )?;
                    println!("{:?}", self.peek());
                    let table_name = match self.parse_primary() {
                        Ok(Value::Varchar(name)) => Ok(name),
                        Err(e) => Err(e),
                        _ => Err(ParseError {
                            message: "Failed to parse 'TABLE' name in 'INSERT' statement"
                                .to_string(),
                        }),
                    }?;
                    println!("{:?}", self.peek());

                    Ok(Statement::Insert { values, table_name })
                }

                _ => Err(ParseError {
                    message: format!("Unexpected token: {:?}", token.tag),
                }),
            },

            None => Err(ParseError {
                message: "Unexpected end of input".to_string(),
            }),
        }
    }

    pub fn parse(tokens: &'a Vec<Token>) -> Result<Statement, ParseError> {
        let mut parser = ParserContext {
            tokens,
            position: 0,
        };

        let statement = parser.parse_statement()?;
        Ok(statement)
    }
}
