# Nabu

A lightweight SQL database engine written from scratch in Rust using B+Trees.

- B-tree storage with page-based disk I/O
- SQL parsing (lexer, parser, type checker)
- Support for `SELECT`, `INSERT`, `DELETE` with `WHERE` clauses
- System table for schema persistence
- Interactive REPL with `.load_database`, `.create_table`, `.exec_file` commands

## Usage

```bash
cargo build --release
cargo run
```

```
.load_database mydb.db
.create_table users id:number name:varchar(32) email:varchar(64)
INSERT INTO users VALUES (1, 'alice', 'alice@example.com')
INSERT INTO users VALUES (2, 'bob', 'bob@example.com')
SELECT * FROM users
SELECT * FROM users WHERE id = 1
SELECT name, email FROM users WHERE id > 1
```

Execute a SQL script file:

```
.load_database mydb.db
.exec_file seed.sql
```

```sql
-- seed.sql
INSERT INTO users VALUES (1, 'alice', 'alice@example.com');
INSERT INTO users VALUES (2, 'bob', 'bob@example.com');
INSERT INTO users VALUES (3, 'carol', 'carol@example.com');
SELECT * FROM users;
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
|   |-- engine.rs         # Query execution
|   +-- evaluator.rs      # Expression evaluation
|
+-- repl.rs
```
