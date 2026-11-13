# Evaluate Embedded DBMS & MS Access Migration Path

Type: `research`
Label: `wayfinder:research`
Status: resolved

## Question

What is the best embedded database engine and migration toolchain to replace a single-user Microsoft Access (`.mdb` / `.accdb`) database on a low-spec Windows 11 PC, developed from macOS?

Investigate against primary sources:

1. **SQLite vs Alternatives (e.g., DuckDB, Limbo/Turso)**:
   - Footprint, query speed for transactional CRUD (forms) + analytical/reporting queries (aggregations, joins), single-file portability, and backup safety (WAL mode, `VACUUM INTO`, online backup API).
   - Rust ecosystem integration (`rusqlite`, `sqlx`) and bundled compilation (`bundled` feature flag for zero external DLL dependency on Windows).
2. **One-time MS Access (`.mdb` / `.accdb`) to SQLite Migration Tools**:
   - How to extract tables, schemas, relationships, and Korean text encodings (CP949 / UTF-16LE in Jet/ACE engines) cleanly on macOS or Windows without data corruption.
   - Compare tools/libraries: `mdbtools`, `sqlite-utils`, Python `pyodbc`/`access-parser`, Jackcess (Java), or exporting via Access/CSV/ODBC.
3. **Saved Queries & Views**:
   - Best practices for translating MS Access saved queries into SQLite Views or parameterized application queries.

Save the detailed findings with primary source citations to `.scratch/access-replacement/research/02-embedded-dbms-and-migration.md`.

## Answer

See the full primary-source evaluation in [02-embedded-dbms-and-migration.md](../research/02-embedded-dbms-and-migration.md):

1. **Embedded DBMS Selection**: **SQLite** via **`rusqlite`** (`features = ["bundled", "backup", "functions"]`) is the clear winner over DuckDB and Turso/libSQL for a low-spec Windows 11 desktop app.
   - **Footprint & Performance**: SQLite adds only ~500 KB–1 MB to the binary and uses ~2–8 MB RAM (vs. DuckDB's 35–60 MB binary overhead and 125 MB–4 GB RAM buffer pool). It executes form-driven row CRUD in microseconds and typical Access-scale reporting joins/aggregations ($10^3–10^6$ rows) in single-digit milliseconds, whereas DuckDB's columnar engine penalizes single-row OLTP updates and Turso's Rust rewrite (`limbo`/`turso`) remains in pre-1.0 beta.
   - **Durability & Safe Single-File Backups**: Run in `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;`. Because raw file copies in WAL mode can miss uncheckpointed `.db-wal` frames, implement one-click user backups with `VACUUM INTO 'backup.db'` (which atomically creates a standalone, defragmented single-file snapshot on a live database) or `rusqlite::backup::Backup` (Online Backup API) if incremental progress reporting is desired.
   - **Rust Integration & Cross-Compilation**: Prefer synchronous `rusqlite` over async `sqlx` to avoid Tokio runtime overhead and gain direct access to SQLite's backup and custom scalar function APIs. Cross-compile `rusqlite` (`bundled`) from macOS to `x86_64-pc-windows-msvc` using `cargo-xwin` (`brew install llvm && cargo install cargo-xwin`), producing a standalone `.exe` with zero external `.dll` dependencies.
2. **One-Time MS Access (`.mdb` / `.accdb`) Migration**:
   - **Toolchain**: Use **`mdbtools`** (`brew install mdbtools` on macOS: `mdb-schema`, `mdb-export --insert sqlite`, `mdb-queries`) or **Jackcess** (JVM) for direct binary-to-SQLite migration. Avoid manual CSV exports via Access/Excel, which corrupt multiline `Memo` fields, strip leading zeros, and trigger CP949/UTF-8 BOM issues on Korean Windows. Avoid Python's `access-parser` (incomplete Memo/OLE support).
   - **Korean Encodings**: Jet 4 (Access 2000+) and ACE (`.accdb`) store text in UTF-16LE and convert automatically to UTF-8 in `mdbtools` and Jackcess. For legacy Jet 3 (Access 97 `.mdb`), set `MDB_JET3_CHARSET="CP949"` before running `mdbtools`.
   - **Data Type Mapping**: Never map Access `Currency` (`MONEY`) to `REAL` (IEEE-754 float); map to `INTEGER` (exact integer KRW `원`, or scaled `× 10000` if fractional decimals exist). Map `Date/Time` to ISO-8601 `TEXT` (`-D '%Y-%m-%d' -T '%Y-%m-%d %H:%M:%S'`), normalize Access `Yes/No` booleans from `-1` to `1`, and inline foreign key constraints in `CREATE TABLE` (since SQLite does not support `ALTER TABLE ADD CONSTRAINT` emitted by `mdb-schema`).
3. **Saved Queries Translation**:
   - Extract saved queries from `MSysQueries` using `mdb-queries` or Jackcess `db.getQueries()`.
   - Convert reusable base join/lookup queries into **SQLite `VIEW`s** (SQLite's query planner flattens views and pushes outer `WHERE` clauses down into indexes), while moving `[Forms]![...]` interactive parameters, `TRANSFORM ... PIVOT` crosstabs (with dynamic columns), and `UPDATE`/`APPEND`/`DELETE` action queries into parameterized/transactional Rust methods.
   - Dialect rewrites: `Nz(a, b)` $\rightarrow$ `COALESCE(a, b)`, `IIF(c, t, f)` $\rightarrow$ native `iif(c, t, f)` (supported since SQLite 3.32.0+) or `CASE WHEN`, `DateAdd()` $\rightarrow$ `date(col, '+N days')`, `TOP N` $\rightarrow$ `LIMIT N`, `&` string concat $\rightarrow$ `concat_ws()` / `||` with `COALESCE`, and `#YYYY-MM-DD#` $\rightarrow$ `'YYYY-MM-DD'`.
