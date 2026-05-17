use super::ast::{
    DeleteStatement, Expression, InsertStatement, Operator, SelectStatement, Statement, Value,
};
use super::lexer::{Token, TokenType};
use ordered_float::OrderedFloat;

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

    fn consume_identifier(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token {
                tag: TokenType::Identifier,
                lexeme,
                ..
            }) => {
                let lexeme = lexeme.clone();
                self.consume();
                Some(lexeme)
            }
            _ => None,
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
                    Ok(Value::Number(OrderedFloat(value)))
                }

                TokenType::String => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Varchar(val.lexeme))
                }

                TokenType::Identifier => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Varchar(val.lexeme))
                }
                TokenType::True => {
                    self.consume();
                    Ok(Value::Bool(true))
                }
                TokenType::False => {
                    self.consume();
                    Ok(Value::Bool(false))
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

    fn get_precedence(&self, tag: &TokenType) -> i8 {
        match tag {
            TokenType::Or => 1,
            TokenType::And => 2,
            TokenType::Equal | TokenType::NotEqual => 3,
            TokenType::Less
            | TokenType::Greater
            | TokenType::LessEqual
            | TokenType::GreaterEqual => 4,
            TokenType::Plus | TokenType::Minus => 5,
            TokenType::Star | TokenType::Slash => 6,
            _ => -1,
        }
    }

    fn token_to_operator(&self, tag: &TokenType) -> Option<Operator> {
        match tag {
            TokenType::Equal => Some(Operator::Eq),
            TokenType::NotEqual => Some(Operator::Neq),
            TokenType::LessEqual => Some(Operator::Leq),
            TokenType::GreaterEqual => Some(Operator::Geq),
            TokenType::Less => Some(Operator::Lt),
            TokenType::Greater => Some(Operator::Gt),
            TokenType::Plus => Some(Operator::Add),
            TokenType::Minus => Some(Operator::Sub),
            TokenType::Star => Some(Operator::Mul),
            TokenType::Slash => Some(Operator::Div),
            TokenType::And => Some(Operator::And),
            TokenType::Or => Some(Operator::Or),
            _ => None,
        }
    }

    fn parse_expr_primary(&mut self) -> Result<Expression, ParseError> {
        match self.peek() {
            Some(token) => match token.tag {
                TokenType::LParen => {
                    self.consume();
                    let expr = self.parse_expression()?;
                    self.consume_assert(TokenType::RParen, "Expected ')' after expression".into())?;
                    Ok(expr)
                }
                TokenType::Number => {
                    let tok = self.consume().unwrap();
                    let value = tok.lexeme.parse::<f64>().map_err(|_| ParseError {
                        message: format!("Failed to parse number: {}", tok.lexeme),
                    })?;
                    Ok(Expression::Literal(Value::Number(OrderedFloat(value))))
                }
                TokenType::String => {
                    let tok = self.consume().unwrap();
                    Ok(Expression::Literal(Value::Varchar(tok.lexeme)))
                }
                TokenType::True => {
                    self.consume();
                    Ok(Expression::Literal(Value::Bool(true)))
                }
                TokenType::False => {
                    self.consume();
                    Ok(Expression::Literal(Value::Bool(false)))
                }
                TokenType::Identifier => {
                    let tok = self.consume().unwrap();
                    Ok(Expression::Identifier(tok.lexeme))
                }
                _ => Err(ParseError {
                    message: format!("Unexpected token in expression: {:?}", token.tag),
                }),
            },
            None => Err(ParseError {
                message: "Unexpected end of input in expression".into(),
            }),
        }
    }

    fn parse_expression_prec(&mut self, min_prec: i8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_expr_primary()?;

        loop {
            let prec = match self.peek() {
                Some(tok) => self.get_precedence(&tok.tag),
                None => -1,
            };

            if prec < min_prec {
                break;
            }

            let op_tok = self.consume().unwrap();
            let op = self
                .token_to_operator(&op_tok.tag)
                .ok_or_else(|| ParseError {
                    message: format!("Unknown operator: {:?}", op_tok.tag),
                })?;

            let rhs = self.parse_expression_prec(prec + 1)?;

            lhs = Expression::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_expression_prec(0)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(token) => match token.tag {
                TokenType::Select => {
                    self.consume();
                    self.consume_optional(TokenType::LParen);

                    let mut columns = vec![];
                    while let Some(name) = self.consume_identifier() {
                        columns.push(name);
                        self.consume_optional(TokenType::Comma);
                    }

                    self.consume_optional(TokenType::RParen);
                    self.consume_assert(TokenType::From, "Expected FROM after SELECT".to_string())?;

                    let table = self.consume_identifier().ok_or_else(|| ParseError {
                        message: "Expected table name after FROM".to_string(),
                    })?;

                    let expr = if self.consume_optional(TokenType::Where).is_some() {
                        Some(Box::new(self.parse_expression()?))
                    } else {
                        None
                    };

                    Ok(Statement::Select(SelectStatement {
                        columns,
                        table,
                        expr,
                    }))
                }

                TokenType::Delete => {
                    self.consume();
                    self.consume_optional(TokenType::LParen);

                    self.consume_assert(TokenType::From, "Expected FROM after SELECT".to_string())?;

                    let table = self.consume_identifier().ok_or_else(|| ParseError {
                        message: "Expected table name after FROM".to_string(),
                    })?;

                    let expr = if self.consume_optional(TokenType::Where).is_some() {
                        Some(Box::new(self.parse_expression()?))
                    } else {
                        None
                    };

                    Ok(Statement::Delete(DeleteStatement { table, expr }))
                }
                TokenType::Insert => {
                    self.consume();
                    let values = self.parse_values()?;
                    self.consume_assert(
                        TokenType::Into,
                        "Expected 'INTO' after 'VALUES' during 'INSERT' statement".to_string(),
                    )?;
                    let table_name = match self.parse_primary() {
                        Ok(Value::Varchar(name)) => Ok(name),
                        Err(e) => Err(e),
                        _ => Err(ParseError {
                            message: "Failed to parse 'TABLE' name in 'INSERT' statement"
                                .to_string(),
                        }),
                    }?;

                    Ok(Statement::Insert(InsertStatement { values, table_name }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::lexer::LexerContext;

    fn parse_expr(input: &str) -> Result<Expression, ParseError> {
        let tokens = LexerContext::lex(input).unwrap();
        let mut parser = ParserContext {
            tokens: &tokens,
            position: 0,
        };
        parser.parse_expression()
    }

    #[test]
    fn test_parse_insert() {
        let tokens = LexerContext::lex("INSERT (1, 2) INTO users").unwrap();
        let stmt = ParserContext::parse(&tokens).unwrap();
        assert!(matches!(stmt, Statement::Insert(_)));
    }

    #[test]
    fn test_parse_insert_with_string() {
        let tokens = LexerContext::lex("INSERT (1, \"hello\") INTO users").unwrap();
        let stmt = ParserContext::parse(&tokens).unwrap();
        if let Statement::Insert(insert) = stmt {
            assert_eq!(insert.values.len(), 2);
            assert_eq!(insert.table_name, "users");
        } else {
            panic!("Expected Insert statement");
        }
    }

    #[test]
    fn test_parse_insert_missing_into() {
        let tokens = LexerContext::lex("INSERT (1, 2) users").unwrap();
        let result = ParserContext::parse(&tokens);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_expr_number() {
        let expr = parse_expr("42").unwrap();
        assert!(matches!(expr, Expression::Literal(Value::Number(n)) if n == 42.0));
    }

    #[test]
    fn test_parse_expr_string() {
        let expr = parse_expr("\"hello\"").unwrap();
        assert!(matches!(expr, Expression::Literal(Value::Varchar(s)) if s == "hello"));
    }

    #[test]
    fn test_parse_expr_identifier() {
        let expr = parse_expr("age").unwrap();
        assert!(matches!(expr, Expression::Identifier(name) if name == "age"));
    }

    #[test]
    fn test_parse_expr_binary_eq() {
        let expr = parse_expr("age == 25").unwrap();
        assert!(matches!(
            expr,
            Expression::BinaryOp {
                op: Operator::Eq,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_expr_binary_and() {
        let expr = parse_expr("age == 25 && name == \"alice\"").unwrap();
        // Top level should be And (lower precedence than ==)
        assert!(matches!(
            expr,
            Expression::BinaryOp {
                op: Operator::And,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_expr_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let expr = parse_expr("1 + 2 * 3").unwrap();
        if let Expression::BinaryOp { op, lhs, rhs } = expr {
            assert!(matches!(op, Operator::Add));
            assert!(matches!(*lhs, Expression::Literal(Value::Number(n)) if n == 1.0));
            assert!(matches!(
                *rhs,
                Expression::BinaryOp {
                    op: Operator::Mul,
                    ..
                }
            ));
        } else {
            panic!("Expected BinaryOp");
        }
    }

    #[test]
    fn test_parse_expr_parens() {
        // (1 + 2) * 3 should parse as (1 + 2) * 3
        let expr = parse_expr("(1 + 2) * 3").unwrap();
        if let Expression::BinaryOp { op, lhs, rhs } = expr {
            assert!(matches!(op, Operator::Mul));
            assert!(matches!(
                *lhs,
                Expression::BinaryOp {
                    op: Operator::Add,
                    ..
                }
            ));
            assert!(matches!(*rhs, Expression::Literal(Value::Number(n)) if n == 3.0));
        } else {
            panic!("Expected BinaryOp");
        }
    }
}
