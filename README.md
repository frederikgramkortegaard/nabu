# Nabu

A lightweight SQL database engine written from scratch in Rust using B+Trees.

- B-tree storage with page-based disk I/O
- SQL parsing (lexer, parser, type checker)
- Support for `SELECT`, `INSERT`, `DELETE` with `WHERE` clauses
- System table for schema persistence

## Usage

```bash
cargo build --release
cargo run
```

```
-- example syntax
INSERT INTO users VALUES (1, 'alice', 'alice@example.com')
INSERT INTO users VALUES (2, 'bob', 'bob@example.com')
SELECT * FROM users
SELECT * FROM users WHERE id = 1
SELECT name, email FROM users WHERE id > 1
```

## Example

```sql
SELECT * FROM users WHERE id > 1
```

```
Ok(Select { rows: [
  [Number(2.0), Varchar("bob"), Varchar("bob@example.com")],
  [Number(3.0), Varchar("carol"), Varchar("carol@example.com")]
] })
```

## Project Structure

```
src/
|-- sql/
|   |-- lexer.rs
|   +-- parser.rs
|
|-- analyzer/
|   |-- bound.rs          # AST binding
|   +-- typechecker.rs
|
|-- storage/
|   |-- btree.rs          # B-tree operations
|   |-- cursor.rs         # Tree traversal
|   |-- node.rs           # Page layout
|   |-- pager.rs          # Disk I/O
|   |-- table.rs          # Schema + B-tree wrapper
|   +-- database.rs       # Multi-table management
|
|-- core/
    |-- engine.rs         # Query execution
    +-- evaluator.rs      # Expression evaluation
```
