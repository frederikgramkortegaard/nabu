use super::ast::*;
use super::lexer::{Token, TokenType};
use crate::error::Error;
use ordered_float::OrderedFloat;

#[derive(Debug, Default)]
pub struct Clauses {
    pub filter: Option<Box<Expression>>,
    pub joins: Vec<Join>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub order_by: Option<QualifiedIdentifier>,
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
    ) -> Result<Token, Error> {
        match self.consume() {
            Some(tok) if tok.tag == expected_type => Ok(tok),
            Some(tok) => Err(Error::Parse(format!(
                "{} at {}:{} (got {:?})",
                message, tok.row, tok.column, tok.tag
            ))),
            None => Err(Error::Parse(format!(
                "{} (unexpected end of input)",
                message
            ))),
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

    /// Parses a potentially qualified identifier (e.g., `column` or `table.column`)
    fn parse_qualified_identifier(&mut self) -> Option<QualifiedIdentifier> {
        let first = self.consume_identifier()?;
        match self.consume_optional(TokenType::Dot) {
            Some(_) => {
                let name = self.consume_identifier()?;
                Some(QualifiedIdentifier {
                    qualifier: Some(first),
                    name,
                })
            }
            None => Some(QualifiedIdentifier {
                qualifier: None,
                name: first,
            }),
        }
    }

    fn parse_clauses(&mut self, allow_join: bool) -> Result<Clauses, Error> {
        let mut clauses = Clauses::default();

        while let Some(token) = self.peek() {
            match &token.tag {
                TokenType::Where => {
                    if clauses.filter.is_some() {
                        return Err(Error::Parse("Duplicate WHERE clause".into()));
                    }
                    self.consume();
                    let expr = self.parse_expression()?;
                    clauses.filter = Some(Box::new(expr));
                }
                TokenType::Join
                | TokenType::InnerJoin
                | TokenType::LeftOuterJoin
                | TokenType::RightOuterJoin
                | TokenType::FullOuterJoin
                | TokenType::CrossJoin => {
                    if !allow_join {
                        return Err(Error::Parse("JOIN not allowed in this statement".into()));
                    }
                    let t = token.tag.clone();
                    self.consume();

                    let Some(table) = self.consume_identifier() else {
                        return Err(Error::Parse("Expected table name after JOIN".into()));
                    };

                    self.consume_assert(TokenType::On, "Expected ON after JOIN".into())?;

                    let on = self.parse_expression()?;

                    let kind = JoinKind::from_token(&t)
                        .ok_or_else(|| Error::Parse("unknown join kind".into()))?;
                    clauses.joins.push(Join {
                        kind,
                        table,
                        on: Box::new(on),
                    });
                }
                TokenType::Limit => {
                    if clauses.limit.is_some() {
                        return Err(Error::Parse("Duplicate LIMIT clause".into()));
                    }
                    self.consume();

                    let limit = self
                        .consume_assert(TokenType::Number, "Expected number after LIMIT".into())?
                        .lexeme
                        .parse::<usize>()
                        .map_err(|_| Error::Parse("LIMIT should be a valid integer".into()))?;

                    if self.consume_optional(TokenType::Comma).is_some() {
                        let offset = self
                            .consume_assert(TokenType::Number, "Expected offset number".into())?
                            .lexeme
                            .parse::<usize>()
                            .map_err(|_| Error::Parse("OFFSET should be a valid integer".into()))?;
                        clauses.offset = Some(offset);
                    }

                    clauses.limit = Some(limit);
                }
                TokenType::OrderBy => {
                    if clauses.order_by.is_some() {
                        return Err(Error::Parse("Duplicate ORDER BY clause".into()));
                    }
                    self.consume();

                    let column = self
                        .parse_qualified_identifier()
                        .ok_or_else(|| Error::Parse("Expected column after ORDER BY".into()))?;
                    clauses.order_by = Some(column);
                }
                _ => break,
            }
        }

        Ok(clauses)
    }

    fn parse_primary(&mut self) -> Result<Value, Error> {
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
                    let value = token.lexeme.parse::<f64>().map_err(|_| {
                        Error::Parse(format!("Failed to parse number: {}", token.lexeme))
                    })?;
                    Ok(Value::Float64(OrderedFloat(value)))
                }

                TokenType::String => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Utf8(val.lexeme))
                }

                TokenType::Identifier => {
                    let val = self.consume().expect("token verified by peek");
                    Ok(Value::Utf8(val.lexeme))
                }
                TokenType::True => {
                    self.consume();
                    Ok(Value::Boolean(true))
                }
                TokenType::False => {
                    self.consume();
                    Ok(Value::Boolean(false))
                }

                _ => Err(Error::Parse(format!(
                    "Unexpected token in expression: {:?}",
                    token.tag
                ))),
            },
            None => Err(Error::Parse(
                "Unexpected end of input in expression".to_string(),
            )),
        }
    }

    fn parse_values(&mut self) -> Result<Vec<Value>, Error> {
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

    fn parse_expr_primary(&mut self) -> Result<Expression, Error> {
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
                    let value = tok.lexeme.parse::<f64>().map_err(|_| {
                        Error::Parse(format!("Failed to parse number: {}", tok.lexeme))
                    })?;
                    Ok(Expression::Literal(Value::Float64(OrderedFloat(value))))
                }
                TokenType::String => {
                    let tok = self.consume().unwrap();
                    Ok(Expression::Literal(Value::Utf8(tok.lexeme)))
                }
                TokenType::True => {
                    self.consume();
                    Ok(Expression::Literal(Value::Boolean(true)))
                }
                TokenType::False => {
                    self.consume();
                    Ok(Expression::Literal(Value::Boolean(false)))
                }
                TokenType::Identifier => {
                    let tok = self.consume().unwrap();
                    match self.consume_optional(TokenType::Dot) {
                        Some(_) => {
                            let name = self.consume_assert(TokenType::Identifier, "".into())?;
                            Ok(Expression::Identifier(QualifiedIdentifier {
                                qualifier: Some(tok.lexeme),
                                name: name.lexeme,
                            }))
                        }
                        None => Ok(Expression::Identifier(QualifiedIdentifier {
                            qualifier: None,
                            name: tok.lexeme,
                        })),
                    }
                }
                _ => Err(Error::Parse(format!(
                    "Unexpected token in expression: {:?}",
                    token.tag
                ))),
            },
            None => Err(Error::Parse("Unexpected end of input in expression".into())),
        }
    }

    fn parse_expression_prec(&mut self, min_prec: i8) -> Result<Expression, Error> {
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
                .ok_or_else(|| Error::Parse(format!("Unknown operator: {:?}", op_tok.tag)))?;

            let rhs = self.parse_expression_prec(prec + 1)?;

            lhs = Expression::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_expression(&mut self) -> Result<Expression, Error> {
        self.parse_expression_prec(0)
    }

    fn parse_statement(&mut self) -> Result<Statement, Error> {
        match self.peek() {
            Some(token) => match token.tag {
                TokenType::Select => {
                    self.consume();
                    self.consume_optional(TokenType::LParen);

                    let mut columns = vec![];
                    while let Some(col) = self.parse_qualified_identifier() {
                        columns.push(col);
                        self.consume_optional(TokenType::Comma);
                    }

                    // @TODO not a very good way of handling * but its okay
                    if columns.is_empty() && self.consume_optional(TokenType::Star).is_some() {
                        columns.push(QualifiedIdentifier {
                            qualifier: None,
                            name: "*".into(),
                        });
                    }

                    self.consume_optional(TokenType::RParen);
                    self.consume_assert(TokenType::From, "Expected FROM after SELECT".to_string())?;

                    let table = self.consume_identifier().ok_or_else(|| {
                        Error::Parse("Expected table name after FROM".to_string())
                    })?;

                    let clauses = self.parse_clauses(true)?;

                    Ok(Statement::Select {
                        table,
                        columns,
                        joins: clauses.joins,
                        filter: clauses.filter,
                        limit: clauses.limit,
                        offset: clauses.offset,
                        order_by: clauses.order_by,
                    })
                }

                TokenType::Delete => {
                    self.consume();
                    self.consume_optional(TokenType::LParen);

                    self.consume_assert(TokenType::From, "Expected FROM after DELETE".to_string())?;

                    let table = self.consume_identifier().ok_or_else(|| {
                        Error::Parse("Expected table name after FROM".to_string())
                    })?;

                    let clauses = self.parse_clauses(false)?;

                    Ok(Statement::Delete {
                        table,
                        filter: clauses.filter,
                    })
                }
                TokenType::Insert => {
                    self.consume();
                    let values = self.parse_values()?;
                    self.consume_assert(
                        TokenType::Into,
                        "Expected 'INTO' after 'VALUES' during 'INSERT' statement".to_string(),
                    )?;
                    let table = match self.parse_primary() {
                        Ok(Value::Utf8(name)) => Ok(name),
                        Err(e) => Err(e),
                        _ => Err(Error::Parse(
                            "Failed to parse 'TABLE' name in 'INSERT' statement".to_string(),
                        )),
                    }?;

                    Ok(Statement::Insert { table, values })
                }

                _ => Err(Error::Parse(format!("Unexpected token: {:?}", token.tag))),
            },

            None => Err(Error::Parse("Unexpected end of input".to_string())),
        }
    }

    pub fn parse(tokens: &'a Vec<Token>) -> Result<Statement, Error> {
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
    use crate::frontend::lexer::LexerContext;

    fn parse_expr(input: &str) -> Result<Expression, Error> {
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
        assert!(matches!(stmt, Statement::Insert { table: _, values: _ }));
    }

    #[test]
    fn test_parse_insert_with_string() {
        let tokens = LexerContext::lex("INSERT (1, \"hello\") INTO users").unwrap();
        let stmt = ParserContext::parse(&tokens).unwrap();
        if let Statement::Insert { table, values } = stmt {
            assert_eq!(values.len(), 2);
            assert_eq!(table, "users");
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
        assert!(matches!(expr, Expression::Literal(Value::Float64(n)) if n == 42.0));
    }

    #[test]
    fn test_parse_expr_string() {
        let expr = parse_expr("\"hello\"").unwrap();
        assert!(matches!(expr, Expression::Literal(Value::Utf8(s)) if s == "hello"));
    }

    #[test]
    fn test_parse_expr_identifier() {
        let expr = parse_expr("age").unwrap();
        assert!(matches!(expr, Expression::Identifier(id) if id.name == "age"));
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
            assert!(matches!(*lhs, Expression::Literal(Value::Float64(n)) if n == 1.0));
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
            assert!(matches!(*rhs, Expression::Literal(Value::Float64(n)) if n == 3.0));
        } else {
            panic!("Expected BinaryOp");
        }
    }
}
