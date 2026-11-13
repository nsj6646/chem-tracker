# Embedded DBMS Evaluation & MS Access Migration Path

## Executive Summary

For a single-user desktop application replacing a Microsoft Access (`.mdb` / `.accdb`) database on a low-spec Windows 11 PC and developed from macOS:

1. **Embedded Database Engine**: **SQLite** (via Rust's `rusqlite` with the `bundled` feature) is the clear winner over DuckDB and Turso/libSQL. It adds only ~500 KB–1 MB to the binary, uses 2–8 MB of RAM by default, executes point-lookup/form CRUD queries in microseconds, handles local reporting joins/aggregations across hundreds of thousands of rows in single-digit milliseconds, and provides atomic single-file snapshot backups via `VACUUM INTO` or incremental backups via the Online Backup API (`sqlite3_backup_*`).
2. **Rust Integration & Cross-Compilation**: Use **`rusqlite`** (`features = ["bundled", "backup"]`) rather than `sqlx`. `rusqlite` compiles the SQLite C amalgamation directly into the `.exe` with zero external `.dll` dependencies, avoids async runtime overhead for local disk I/O, and cross-compiles cleanly from macOS to `x86_64-pc-windows-msvc` via `cargo-xwin` (`brew install llvm && cargo install cargo-xwin`).
3. **One-Time MS Access Migration**: Use **`mdbtools`** (`brew install mdbtools` on macOS) or **Jackcess** (JVM) for direct binary extraction from `.mdb`/`.accdb` to SQLite, bypassing intermediate CSV files wherever possible to avoid multiline `Memo` corruption, type erasure, and Excel CP949/UTF-8 BOM pitfalls. Map Access `Currency` to integer minor units (`INTEGER`—standard for KRW `원`) or scaled integers (`× 10000`) to prevent IEEE-754 float rounding errors.
4. **Saved Queries Translation**: Extract Access saved queries using `mdb-queries` (or Jackcess `db.getQueries()`). Convert standard `SELECT` queries into **SQLite `VIEW`s** (rewriting `Nz()` $\rightarrow$ `COALESCE()`, `DateAdd()` $\rightarrow$ `datetime()`, `TOP N` $\rightarrow$ `LIMIT N`, while `IIF()` is natively supported in SQLite 3.32.0+). Convert `[Forms]![...]` interactive filter queries, Crosstab (`TRANSFORM ... PIVOT`) queries, and Action queries (`UPDATE`/`INSERT`/`DELETE`) into parameterized Rust application queries.

---

## 1. Embedded DBMS Comparison: SQLite vs. DuckDB vs. Turso / libSQL

### 1.1 Architectural Comparison Table

| Dimension | SQLite (3.45+) | DuckDB (1.x) | Turso / Limbo & libSQL |
| :--- | :--- | :--- | :--- |
| **Primary Workload** | Row-oriented OLTP (transactional CRUD) + lightweight local OLAP | Columnar vectorized OLAP (heavy analytical scans, Parquet/CSV) | Edge/cloud-replicated OLTP (`libSQL` C fork) / Next-gen async rewrite (`turso` Rust) |
| **Binary Footprint** | **~500 KB – 1 MB** compiled into executable | **~35 MB – 60+ MB** added to binary (`libduckdb` C++) | `libSQL`: ~2–5 MB; `turso` (Limbo): Pure Rust, evolving |
| **RAM Footprint** | **~250 KB base + ~2–8 MB** page cache (`PRAGMA cache_size = -2000`) | **125 MB minimum per thread**; typically **1–4 GB RAM** buffer pool | Similar to SQLite (`libSQL`), plus HTTP/sync runtime overhead if enabled |
| **Form CRUD Latency** | **Sub-millisecond ($\mu\text{s}$)** B-tree row point lookups & single-row writes | **Poor for OLTP**: row mutations use HyPer-style MVCC undo buffers; slow single-row updates & FK checks | Fast (`libSQL` matches SQLite; `turso` Rust rewrite still maturing) |
| **Reporting / Joins** | **1–20 ms** for typical Access-scale datasets ($10^3–10^6$ rows) with proper indexes | **Ultra-fast** on $10^7–10^9$ rows; overkill for typical Access dataset sizes | Matches SQLite (`libSQL`); `turso` Rust optimizer still incomplete |
| **Production Status** | **Rock-solid** (Aviation-grade MC/DC test suite, 20+ years production) | **Production-ready for OLAP**; explicitly discourages high-frequency row-by-row OLTP | `libSQL` (C) is production-ready; `turso`/Limbo (Rust) is **Beta / Pre-1.0** |
| **macOS $\rightarrow$ Win11 Cross-Compile** | **Trivial** (single C file `sqlite3.c` via `cc` + `cargo-xwin`) | **Slow / Heavy** (`duckdb-rs` `bundled` compiles a massive C++17 codebase or requires external `duckdb.dll`) | `libsql` pulls C + Tokio/hyper/TLS stacks; `turso` uses `io_uring`/system specifics |

### 1.2 Why SQLite Wins for Low-Spec Windows 11 Access Replacement

1. **Memory and Disk Discipline on Low-Spec Hardware**:
   - SQLite's core library requires ~250 KB of RAM and defaults to a 2,000-page cache (`PRAGMA cache_size = -2000`, ~2 MB to 8 MB RAM).([sqlite.org/footprint.html](https://www.sqlite.org/footprint.html), [sqlite.org/pragma.html#pragma_cache_size](https://www.sqlite.org/pragma.html#pragma_cache_size))
   - By contrast, DuckDB's vectorized execution engine allocates a minimum of ~125 MB per thread and targets 1–4 GB of RAM, and compiling `duckdb` with `bundled` bloats the binary by tens of megabytes and adds minutes to clean compile times.([duckdb.org/docs/operations_manual/footprint_of_duckdb](https://duckdb.org/docs/operations_manual/footprint_of_duckdb), [docs.rs/duckdb](https://docs.rs/duckdb/latest/duckdb/))
2. **Workload Fit (Forms + Local Reports)**:
   - MS Access applications are dominated by interactive forms (point lookups by ID, scrolling records, inserting/updating individual line items) paired with summary reports over thousands or tens of thousands of rows.
   - SQLite is explicitly marketed by its authors as a direct replacement for MS Access and `fopen()`, excelling at B-tree point lookups and transactional writes while easily computing joins and `GROUP BY` aggregations over $10^5$ rows in milliseconds.([sqlite.org/whentouse.html](https://www.sqlite.org/whentouse.html))
   - Turso's Rust rewrite (`tursodatabase/turso`, formerly `limbo`) is explicitly marked **Beta / not yet 1.0** for mission-critical production persistence, while `libSQL`'s primary value proposition (HTTP remote replicas, multi-tenant cloud sync) is unnecessary weight for a standalone single-PC desktop app.([github.com/tursodatabase/turso](https://github.com/tursodatabase/turso), [github.com/tursodatabase/libsql](https://github.com/tursodatabase/libsql))

### 1.3 Durability, Concurrency, and Safe Single-File Backups in SQLite

MS Access `.mdb`/`.accdb` files are notoriously prone to index/page corruption when copied while open or interrupted during writes. SQLite eliminates this risk with three built-in mechanisms:

1. **WAL Mode (`PRAGMA journal_mode = WAL;`)**:
   - Appends changes sequentially to a write-ahead log (`.db-wal`) with a small shared-memory index (`.db-shm`). Readers never block writers, and `PRAGMA synchronous = NORMAL;` in WAL mode provides full crash-safe durability without calling `FlushFileBuffers` on every single transaction commit, greatly improving responsiveness on low-spec Windows PCs with slower disks.([sqlite.org/wal.html](https://www.sqlite.org/wal.html))
   - **Important Caveat**: While the app is running in WAL mode, a raw Windows Explorer file copy of *only* `app.db` (omitting `app.db-wal`) can miss uncheckpointed transactions or produce a torn state.([sqlite.org/howtocorrupt.html](https://www.sqlite.org/howtocorrupt.html))
2. **`VACUUM INTO 'backup_path.db'` (SQLite 3.27.0+)**:
   - The simplest, safest "Export / Save Backup Copy" button implementation for a single-user desktop app.
   - Executes atomically on a live WAL-mode database (`conn.execute("VACUUM INTO ?1", [backup_path])`) and writes a completely self-contained, defragmented, single-file `.db` snapshot (in standard rollback mode with zero `.db-wal` residue) ready to be copied to a USB drive or network folder.([sqlite.org/lang_vacuum.html#vacuuminto](https://www.sqlite.org/lang_vacuum.html#vacuuminto))
3. **Online Backup API (`sqlite3_backup_init` / `rusqlite::backup`)**:
   - Exposed in `rusqlite` via the `backup` feature flag (`rusqlite::backup::Backup`).
   - Copies pages incrementally between a live source `Connection` and a destination `Connection` with configurable chunk size and sleep intervals, providing a progress callback (`backup::Progress { remaining, pagecount }`) if the UI needs a progress bar for very large databases.([sqlite.org/backup.html](https://www.sqlite.org/backup.html), [docs.rs/rusqlite/latest/rusqlite/backup/index.html](https://docs.rs/rusqlite/latest/rusqlite/backup/index.html))

### 1.4 Rust Ecosystem (`rusqlite` vs. `sqlx`) & Cross-Compilation (`macOS` $\rightarrow$ `Windows 11`)

#### `rusqlite` vs. `sqlx`

| Feature | `rusqlite` | `sqlx` |
| :--- | :--- | :--- |
| **Execution Model** | **Synchronous** (direct C FFI call, zero runtime overhead) | **Asynchronous** (requires Tokio/async-std; spawns background worker thread to bridge synchronous SQLite C calls) |
| **SQLite Feature Coverage** | **100% First-Class**: Online Backup API, custom scalar/aggregate functions, custom collations, virtual tables, hooks | **Lowest-common-denominator** relational abstraction; advanced SQLite C APIs require dropping down to raw handles |
| **Compile-time Checking** | Runtime-checked SQL (testable via standard unit tests against `:memory:`) | `sqlx::query!` macro requires a live `DATABASE_URL` SQLite file or committed `.sqlx` offline metadata during compilation |
| **Verdict** | **Recommended** for single-user desktop apps | Unnecessary async complexity unless forced by framework |

*(Sources: [github.com/rusqlite/rusqlite](https://github.com/rusqlite/rusqlite), [github.com/launchbadge/sqlx](https://github.com/launchbadge/sqlx))*

#### Recommended `Cargo.toml` Configuration

```toml
[dependencies]
rusqlite = { version = "0.32", features = [
    "bundled",          # Compiles latest SQLite C amalgamation statically into the binary (zero .dll dependency)
    "backup",           # Exposes sqlite3_backup_* Online Backup API
    "functions",        # Allows registering custom Rust scalar functions (e.g., Access compatibility helpers)
    "modern_sqlite",    # Enables latest SQLite features
] }
```

#### Cross-Compiling from macOS to Windows 11 (`x86_64-pc-windows-msvc`)

Because `rusqlite` with `bundled` compiles `sqlite3.c` via the `cc` crate (`libsqlite3-sys`), cross-compiling from macOS to `x86_64-pc-windows-msvc` requires an MSVC-compatible C compiler (`clang-cl`), linker (`lld-link`), and Windows SDK headers. **`cargo-xwin`** automates this cleanly without needing a Windows VM:

```bash
# 1. Install LLVM (provides clang-cl and lld-link) and cargo-xwin on macOS
brew install llvm
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
cargo install --locked cargo-xwin

# 2. Add the Windows 11 x86_64 MSVC target
rustup target add x86_64-pc-windows-msvc

# 3. Build standalone .exe (automatically downloads Windows CRT/SDK headers on first run)
XWIN_ACCEPT_LICENSE=1 cargo xwin build --target x86_64-pc-windows-msvc --release
```

*(Sources: [github.com/rust-cross/cargo-xwin](https://github.com/rust-cross/cargo-xwin), [github.com/rusqlite/rusqlite#notes-on-building-rusqlite-and-libsqlite3-sys](https://github.com/rusqlite/rusqlite#notes-on-building-rusqlite-and-libsqlite3-sys))*

#### Recommended Connection Initialization Pragmas

Run these pragmas every time the application opens the database connection:

```sql
PRAGMA journal_mode = WAL;          -- Concurrent reads, fast crash-safe writes
PRAGMA synchronous = NORMAL;        -- Safe durability in WAL mode without excessive fsync stalls
PRAGMA foreign_keys = ON;           -- SQLite disables FK enforcement by default; must enable per connection!
PRAGMA busy_timeout = 5000;         -- Wait up to 5s on lock contention instead of returning SQLITE_BUSY immediately
PRAGMA temp_store = MEMORY;         -- Keep temporary sort/index tables in RAM
PRAGMA cache_size = -8000;          -- 8 MB page cache
```

*(Source: [sqlite.org/pragma.html](https://www.sqlite.org/pragma.html))*

---

## 2. One-Time MS Access (`.mdb` / `.accdb`) to SQLite Migration

### 2.1 Korean Text Encodings in Jet 3, Jet 4, and ACE Engines

Preventing Korean character corruption (Mojibake / `깨짐`) requires understanding how MS Access stores strings across engine generations:

1. **Jet 3 (`.mdb` — Access 97 and earlier)**:
   - Stores text in a single/multi-byte ANSI code page without Unicode. Korean Windows databases use **CP949 (Unified Hangul Code / EUC-KR superset)**.
   - By default, `mdbtools` assumes Jet 3 files use `CP1252` (Western European). You **must** set `MDB_JET3_CHARSET="CP949"` in the environment before running `mdbtools` on a Jet 3 `.mdb` file.([github.com/mdbtools/mdbtools](https://github.com/mdbtools/mdbtools))
2. **Jet 4 (`.mdb` — Access 2000/2002/2003) & ACE (`.accdb` — Access 2007+)**:
   - Store all text internally as **UTF-16LE** (with Microsoft's standard Unicode BOCU-like single-byte compression for ASCII ranges).
   - Both `mdbtools` (via `iconv`, defaulting to `MDBICONV="UTF-8"`) and `Jackcess` natively decompress and transcode Jet 4 / ACE UTF-16LE strings into clean **UTF-8** without needing manual code-page overrides.([github.com/mdbtools/mdbtools](https://github.com/mdbtools/mdbtools), [jackcess.sourceforge.io](https://jackcess.sourceforge.io/))
3. **Why Intermediate CSV Export via MS Access / Excel Is Dangerous on Korean Windows**:
   - Exporting CSV from MS Access on Korean Windows defaults to `CP949` (`EUC-KR`) unless UTF-8 is explicitly chosen, while Excel on Windows requires a UTF-8 BOM (`EF BB BF`) to recognize UTF-8 CSVs.
   - Worse, MS Access `Memo` (Long Text) fields frequently contain embedded `\r\n` newlines, double quotes, and commas that break naïve CSV parsers, and leading zeros in Korean postal codes, business registration numbers (`사업자등록번호`), or phone numbers are silently stripped if opened in Excel or auto-inferred as integers.
   - **Rule**: Migrate **directly from `.mdb`/`.accdb` binary to SQLite** (or via typed SQL `INSERT` statements / JDBC / DB-API), never through manual Excel/CSV round-trips.

### 2.2 Comparing Migration Toolchains

| Tool / Library | Host OS | Format Support | Schema & FKs | Saved Queries | Pros & Cons |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`mdbtools`** (`brew install mdbtools`) | **macOS**, Linux | Jet 3/4 `.mdb`, ACE `.accdb` | `mdb-schema --relations` | `mdb-queries` extracts SQL | **Best native macOS CLI option.** Fast C implementation; exports SQLite DDL, direct SQL `INSERT`s, or JSON/CSV. Caveat: `mdb-schema` outputs `ALTER TABLE ADD CONSTRAINT` for FKs, which SQLite rejects—FKs must be inline in `CREATE TABLE`. |
| **Jackcess** (Java / Kotlin / UCanAccess) | **macOS**, Win, Linux (JVM) | Access 97–2019 (`.mdb`, `.accdb`) | Full `Table`, `Column`, `Relationship` API | `db.getQueries()` parses `MSysQueries` | **Highest fidelity cross-platform parser.** Pure Java, handles compressed Memo fields, exact `BigDecimal` Currency, complex types, and `MSysQueries` decompilation cleanly. |
| **Python `pyodbc`** (+ MS ACE ODBC Driver) | **Windows only** | `.mdb`, `.accdb` | Via `crsr.tables()`, `crsr.statistics()` | Via COM / DAO (`win32com`) | Uses Microsoft's official engine—100% accurate data reading. Requires Windows and matching 32-bit/64-bit MS Access Database Engine Redistributable. |
| **`sqlite-utils`** | macOS, Win, Linux | JSON / CSV / TSV (No native `.mdb`) | `--pk`, `--fk`, `transform` | N/A | Great post-processing CLI (`mdb-export` $\rightarrow$ `sqlite-utils insert`), but relies on string type inference if fed CSVs. |
| **Python `access-parser`** | macOS, Win, Linux | `.mdb`, `.accdb` | Partial | No | **Not recommended.** Pure-Python reverse-engineered parser; fails on overflow `Memo` pages, OLE blobs, and many index/page layouts. |

*(Sources: [github.com/mdbtools/mdbtools](https://github.com/mdbtools/mdbtools), [jackcess.sourceforge.io](https://jackcess.sourceforge.io/), [github.com/mkleehammer/pyodbc](https://github.com/mkleehammer/pyodbc), [sqlite-utils.datasette.io](https://sqlite-utils.datasette.io/), [github.com/ClarotyICS/access_parser](https://github.com/ClarotyICS/access_parser))*

### 2.3 MS Access to SQLite Data Type Mapping & Pitfalls

SQLite uses dynamic type affinity (`INTEGER`, `REAL`, `TEXT`, `BLOB`, `NUMERIC`), optionally enforced via `STRICT` tables (introduced in SQLite 3.37.0).([sqlite.org/datatype3.html](https://www.sqlite.org/datatype3.html), [sqlite.org/stricttables.html](https://www.sqlite.org/stricttables.html)) Every Access type must be mapped deliberately:

| MS Access Data Type | Internal Access Storage | Recommended SQLite Type | Critical Migration Rule |
| :--- | :--- | :--- | :--- |
| **AutoNumber** (Long) | 32-bit signed integer | `INTEGER PRIMARY KEY` | Preserve existing IDs during import so foreign keys remain intact. Use `INTEGER PRIMARY KEY` (rowid alias); avoid `AUTOINCREMENT` unless strict monotonic non-reuse after deletion is required.([sqlite.org/autoinc.html](https://www.sqlite.org/autoinc.html)) |
| **Currency** (`MONEY`) | 64-bit integer scaled by $10,000$ (4 fixed decimal places) | **`INTEGER`** | **NEVER store Currency as `REAL` (IEEE-754 float).** For Korean Won (`KRW` / `원`), store exact integer Won (`INTEGER`). If fractional sub-units ( up to 4 decimals) exist, store scaled integer (`amount_x10000 INTEGER`) or exact string (`TEXT`) and compute via `rust_decimal`. |
| **Date/Time** | 64-bit IEEE float (days since `1899-12-30 00:00:00`) | **`TEXT`** (ISO-8601) | Format as `'YYYY-MM-DD HH:MM:SS'` or `'YYYY-MM-DD'`. Beware Access's `1899-12-30` epoch (Lotus 1-2-3 leap-year bug): time-only values in Access appear as dates on `1899-12-30`. Convert using `mdb-export -D '%Y-%m-%d' -T '%Y-%m-%d %H:%M:%S'`.([sqlite.org/lang_datefunc.html](https://www.sqlite.org/lang_datefunc.html)) |
| **Yes/No** (Boolean) | 1-bit (`-1` = True, `0` = False in Access/VBA!) | **`INTEGER NOT NULL CHECK (col IN (0, 1))`** | **Gotcha**: Access SQL/VBA evaluates `True` as `-1` and `False` as `0`. Normalize all `-1` values to `1` during migration so SQLite checks `WHERE is_active = 1`. |
| **Short Text** (`TEXT`) | UTF-16LE (or CP949 in Jet 3), $\le 255$ chars | `TEXT` | Transcode to UTF-8. Strip trailing NUL bytes if any exist from legacy Jet 3 fixed buffers. |
| **Long Text / Memo** | Multi-page LVAL chains | `TEXT` | SQLite `TEXT` supports up to $1\text{ GB}$ by default in ordinary rows—no special Memo handling needed once extracted cleanly via `mdbtools` (`--insert sqlite`) or Jackcess. |
| **Number (Byte / Integer / Long)** | 1 / 2 / 4-byte integer | `INTEGER` | Direct 1:1 mapping. |
| **Number (Decimal / Numeric)** | 17-byte scaled decimal | `TEXT` or `INTEGER` (scaled) | Beware SQLite `NUMERIC` affinity: SQLite converts `NUMERIC` values with decimals into 64-bit `REAL` floats internally! Store as scaled `INTEGER` or `TEXT` if exact decimal precision is required.([sqlite.org/datatype3.html](https://www.sqlite.org/datatype3.html)) |
| **Number (Single / Double)** | 32-bit / 64-bit IEEE-754 float | `REAL` | Direct mapping for scientific/non-monetary floats. |
| **OLE Object / Attachment** | Proprietary OLE wrapper + binary payload | `BLOB` or external file path (`TEXT`) | Access wraps embedded files in an OLE header. Use Jackcess `OleUtil` if unwrapping embedded files/images is needed. |

### 2.4 Recommended Repeatable Migration Pipeline (macOS Development Workflow)

For developing and testing on macOS, combine `mdbtools` with a deterministic Python or Rust migration script:

```bash
# 1. Install mdbtools on macOS
brew install mdbtools

# 2. Inspect tables, relationships, and saved queries
mdb-tables -1 legacy.accdb
mdb-schema --relations legacy.accdb sqlite > raw_schema.sql
mdb-queries -L legacy.accdb

# 3. Export data directly as SQLite INSERT statements (avoids CSV newline/quoting bugs!)
#    Use -D and -T to enforce ISO-8601 dates and timestamps.
#    (If legacy.mdb is Access 97 / Jet 3, prefix with MDB_JET3_CHARSET="CP949")
for tbl in $(mdb-tables -1 legacy.accdb); do
    mdb-export \
        -D '%Y-%m-%d' \
        -T '%Y-%m-%d %H:%M:%S' \
        --insert sqlite \
        legacy.accdb "$tbl" > "dump_${tbl}.sql"
done
```

**Schema Refinement Step**:
1. Take `raw_schema.sql` and rewrite the target SQLite DDL with explicit `PRIMARY KEY`, inline `REFERENCES parent(id)` foreign keys (since SQLite does not support `ALTER TABLE ... ADD CONSTRAINT FOREIGN KEY` emitted by `mdb-schema`), and `STRICT` table modifiers where helpful.
2. Wrap each `dump_${tbl}.sql` inside `BEGIN TRANSACTION; ... COMMIT;` and load in parent-Before-child topological order (or set `PRAGMA defer_foreign_keys = ON;` during transaction load, then run `PRAGMA foreign_key_check;` before commit to verify referential integrity).([sqlite.org/pragma.html#pragma_foreign_key_check](https://www.sqlite.org/pragma.html#pragma_foreign_key_check))

---

## 3. Translating MS Access Saved Queries to SQLite Views & Application Queries

### 3.1 Extracting Saved Queries from `.mdb` / `.accdb`

MS Access stores saved queries in the hidden system catalog table `MSysQueries` (linked to `MSysObjects` where `Type = 5`). You do not need MS Access installed to extract them:

- **Using `mdbtools` on macOS**:
  ```bash
  # List all saved query names
  mdb-queries -L legacy.accdb

  # Dump the SQL of every saved query
  for q in $(mdb-queries -L legacy.accdb); do
      echo "--- QUERY: $q ---"
      mdb-queries legacy.accdb "$q"
  done
  ```
- **Using Jackcess (JVM)**:
  Call `db.getQueries()`, which parses `MSysQueries` into `Query` subclasses (`SelectQuery`, `AppendQuery`, `UpdateQuery`, `DeleteQuery`, `CrossTabQuery`, `MakeTableQuery`) and reconstructs the SQL string via `query.toSQLString()`.([jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/query/Query.html](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/query/Query.html))

### 3.2 Architectural Decision: SQLite `VIEW` vs. Parameterized Application Query

SQLite `VIEW`s are read-only, non-parameterized virtual tables (essentially saved `SELECT` ASTs that get merged into the outer query by the SQLite query planner).([sqlite.org/lang_createview.html](https://www.sqlite.org/lang_createview.html)) Classify every Access saved query into one of four buckets:

| Access Saved Query Type | Target in New Architecture | Rationale |
| :--- | :--- | :--- |
| **1. Base Join / Composable Lookup Queries** (e.g., `qryOrderDetailsWithCustomer`, `qryActiveInventory`) | **SQLite `CREATE VIEW`** | SQLite's query planner flattens views (`SQLITE_QueryFlattener`) and pushes outer `WHERE` predicates down into indexed base tables automatically. Keeps Rust code clean and DRY. |
| **2. Interactive Parameter / Form-Bound Queries** (e.g., `WHERE OrderDate >= [Forms]![frmReport]![txtStart]`) | **Parameterized SQL in Rust** (`?1`, `?2` or `:start_date`) | SQLite Views cannot accept parameters. Either query an underlying SQLite `VIEW` with a `WHERE` clause from Rust, or execute a prepared statement in `rusqlite`. |
| **3. Crosstab Queries (`TRANSFORM ... PIVOT`)** | **Static `CASE WHEN` View** or **Dynamic Rust Pivot** | SQLite has no `PIVOT` syntax. If pivot columns are fixed (e.g., `Q1..Q4` or `M01..M12`), use `SUM(CASE WHEN ... END)` in a View; if columns are dynamic (e.g., arbitrary customer names), aggregate with `GROUP BY row_key, col_key` in SQLite and pivot into a grid struct in Rust. |
| **4. Action Queries (`UPDATE`, `APPEND`, `DELETE`, `MAKE TABLE`)** | **Transactional Rust Service Methods** (`conn.transaction()`) | Replace saved action queries (and macros that chain them) with explicit, unit-tested Rust functions running inside `rusqlite::Transaction`. |

### 3.3 Access SQL (Jet/ACE) $\rightarrow$ SQLite Dialect Translation Reference

| Access SQL Construct | Access Example | SQLite Equivalent | Notes |
| :--- | :--- | :--- | :--- |
| **Null Coalescing (`Nz`)** | `Nz([Discount], 0)` | `COALESCE(Discount, 0)` or `IFNULL(Discount, 0)` | `Nz()` is a VBA/Access host function, not standard SQL. |
| **Conditional (`IIF`)** | `IIF(Qty > 10, 'Bulk', 'Std')` | `iif(Qty > 10, 'Bulk', 'Std')` or `CASE WHEN Qty > 10 THEN 'Bulk' ELSE 'Std' END` | **Native in SQLite 3.32.0+!** SQLite added built-in `iif(X,Y,Z)` (and 4+ arg `iif()` in 3.48.0).([sqlite.org/lang_corefunc.html#iif](https://www.sqlite.org/lang_corefunc.html#iif)) |
| **Row Limiting (`TOP`)** | `SELECT TOP 10 * FROM Sales` | `SELECT * FROM Sales LIMIT 10` | Beware: Access `SELECT TOP 10` *includes ties* by default when `ORDER BY` is present! |
| **Date Literals (`#...#`)** | `WHERE OrderDate >= #2025-01-01#` | `WHERE OrderDate >= '2025-01-01'` | Replace `#` delimiters with ISO-8601 single-quoted strings. |
| **Current Date/Time** | `Date()` / `Now()` | `date('now', 'localtime')` / `datetime('now', 'localtime')` | SQLite `'now'` defaults to UTC; pass `'localtime'` to match Access's local PC clock behavior.([sqlite.org/lang_datefunc.html](https://www.sqlite.org/lang_datefunc.html)) |
| **Date Arithmetic (`DateAdd`)** | `DateAdd('d', 30, [OrderDate])` | `date(OrderDate, '+30 days')` | Modifiers: `'+N days'`, `'+N months'`, `'+N years'`. |
| **Date Difference (`DateDiff`)** | `DateDiff('d', [Start], [End])` | `CAST(julianday(End) - julianday(Start) AS INTEGER)` | `julianday()` returns fractional days since noon Nov 24, 4714 BC. |
| **Date Parts (`Year`, `Month`, `Day`)** | `Year([OrderDate])`, `Month([OrderDate])` | `CAST(strftime('%Y', OrderDate) AS INTEGER)`, `CAST(strftime('%m', OrderDate) AS INTEGER)` | `strftime()` returns `TEXT`; cast to `INTEGER` if doing numeric comparisons. |
| **String Concatenation** | `FirstName & ' ' & LastName` | `FirstName \|\| ' ' \|\| LastName` | Note: In Access, `'A' & NULL` evaluates to `'A'`, whereas in SQLite `'A' \|\| NULL` is `NULL`! Wrap nullable columns in `COALESCE(col, '')` or use `concat_ws(' ', FirstName, LastName)` (SQLite 3.44.0+).([sqlite.org/lang_corefunc.html#concat_ws](https://www.sqlite.org/lang_corefunc.html#concat_ws)) |
| **String Functions** | `Mid(s, start, len)`, `InStr(s, sub)`, `Len(s)`, `UCase(s)` | `substr(s, start, len)`, `instr(s, sub)`, `length(s)`, `upper(s)` | Both Access `Mid()` and SQLite `substr()` are 1-indexed. |
| **Wildcards (`LIKE` / `ALIKE`)** | `LIKE 'Kim*'` or `LIKE '202?-'` | `LIKE 'Kim%'` or `LIKE '202_-'` | Access DAO uses `*` and `?`; ANSI SQL / SQLite uses `%` and `_`. |
| **Boolean Literals** | `WHERE IsActive = True` (or `-1`) | `WHERE IsActive = 1` | Normalize `-1` to `1` during data migration. |
| **Nested `INNER JOIN` Parentheses** | `FROM ((A INNER JOIN B ON ...) INNER JOIN C ON ...)` | `FROM A JOIN B ON ... JOIN C ON ...` | SQLite accepts parenthesized joins too, but unnesting improves readability. |
| **First / Last Aggregates** | `SELECT First(Note) ... GROUP BY CustomerID` | `SELECT Note ...` with `MIN(rowid)` / Window Function `FIRST_VALUE()` | Access `First()`/`Last()` return arbitrary row order unless sorted; replace with deterministic window functions or `GROUP BY` semantics. |
| **Crosstab (`TRANSFORM ... PIVOT`)** | `TRANSFORM Sum(Amt) SELECT Cust FROM Sales GROUP BY Cust PIVOT Month` | `SELECT Cust, SUM(iif(Month=1, Amt, 0)) AS M1, ... FROM Sales GROUP BY Cust` | Use static conditional aggregation in SQLite or dynamic pivoting in Rust. |

*(Sources: [sqlite.org/lang_corefunc.html](https://www.sqlite.org/lang_corefunc.html), [sqlite.org/lang_datefunc.html](https://www.sqlite.org/lang_datefunc.html), [learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/microsoft-access-sql-reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/microsoft-access-sql-reference))*

### 3.4 Pro-Tip: Registering Custom SQLite Scalar Functions in `rusqlite`

Because `rusqlite` supports registering custom Rust closures as SQLite scalar functions (`Connection::create_scalar_function` via the `functions` feature), if the legacy Access database has complex custom VBA functions (or if you want drop-in functions like `nz()` or custom Korean formatting helpers inside SQLite Views), you can register them directly in Rust in a few lines of code([docs.rs/rusqlite/latest/rusqlite/functions/index.html](https://docs.rs/rusqlite/latest/rusqlite/functions/index.html)):

```rust
conn.create_scalar_function(
    "date_diff_days",
    2,
    rusqlite::functions::FunctionFlags::SQLITE_UTF8 | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
    |ctx| {
        let start: String = ctx.get(0)?;
        let end: String = ctx.get(1)?;
        // Custom date parsing / business day logic in Rust
        Ok(0i64)
    },
)?;
```
