//! Shared domain normalization, validation, and transactional upsert helpers
//! reused across products, shipments, masters, excel imports, reports, and backups.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};

/// Normalizes a CAS number by trimming surrounding whitespace, converting Unicode dashes
/// (`\u{2010}`..=`\u{2015}`, `\u{2212}`) to ASCII `-`, removing spaces around `-`, and
/// collapsing remaining consecutive whitespace to single ASCII spaces.
pub fn normalize_cas_number(raw: &str) -> String {
    let unified_dashes = raw.trim().replace(
        ['\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}', '\u{2015}', '\u{2212}'],
        "-",
    );
    let mut out = String::with_capacity(unified_dashes.len());
    let mut prev_space = false;
    for ch in unified_dashes.chars() {
        if ch.is_whitespace() {
            prev_space = true;
            continue;
        }
        if ch == '-' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push('-');
            prev_space = false;
            continue;
        }
        if prev_space && !out.is_empty() && !out.ends_with('-') {
            out.push(' ');
        }
        out.push(ch);
        prev_space = false;
    }
    out
}

/// Returns the number of days in `month` (1..=12) for Gregorian `year`.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Validates a strict `YYYY-MM-DD` calendar date string and returns `(year, month, day)`.
pub fn validate_calendar_date(raw: &str) -> AppResult<(i32, u32, u32)> {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[0..4].iter().all(|b| b.is_ascii_digit())
        || !bytes[5..7].iter().all(|b| b.is_ascii_digit())
        || !bytes[8..10].iter().all(|b| b.is_ascii_digit())
    {
        return Err(AppError::Validation(format!(
            "날짜는 YYYY-MM-DD 형식이어야 합니다: '{raw}'"
        )));
    }

    let year: i32 = trimmed[0..4].parse().unwrap();
    let month: u32 = trimmed[5..7].parse().unwrap();
    let day: u32 = trimmed[8..10].parse().unwrap();

    if !(1..=12).contains(&month) {
        return Err(AppError::Validation(format!(
            "올바른 월(01~12)이 아닙니다: '{raw}'"
        )));
    }

    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(AppError::Validation(format!(
            "존재하지 않는 날짜입니다: '{raw}'"
        )));
    }

    Ok((year, month, day))
}

/// Finds an existing Overseas Branch (`해외지사`) case-insensitively or inserts a new row.
pub fn upsert_overseas_branch(conn: &Connection, name: &str) -> AppResult<i64> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("해외지사명이 비어 있습니다.".to_string()));
    }
    if let Some(existing_id) = conn
        .query_row(
            "SELECT id FROM overseas_branches WHERE name = ?1 COLLATE NOCASE;",
            [trimmed],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(existing_id);
    }

    conn.execute("INSERT INTO overseas_branches (name) VALUES (?1);", [trimmed])?;
    Ok(conn.last_insert_rowid())
}

/// Finds an existing Domestic Importer (`국내 수입사`) case-insensitively or inserts a new row.
pub fn upsert_domestic_importer(conn: &Connection, name: &str) -> AppResult<i64> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("국내 수입사명이 비어 있습니다.".to_string()));
    }
    if let Some(existing_id) = conn
        .query_row(
            "SELECT id FROM domestic_importers WHERE name = ?1 COLLATE NOCASE;",
            [trimmed],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(existing_id);
    }

    conn.execute("INSERT INTO domestic_importers (name) VALUES (?1);", [trimmed])?;
    Ok(conn.last_insert_rowid())
}

/// Finds an existing Chemical Substance (`화학물질`) by normalized CAS number or inserts one.
/// Preserves an existing non-empty `substance_name` if the incoming name is empty or `None`.
pub fn upsert_chemical_substance(
    conn: &Connection,
    cas_number: &str,
    substance_name: Option<&str>,
) -> AppResult<i64> {
    let normalized_cas = normalize_cas_number(cas_number);
    if normalized_cas.is_empty() {
        return Err(AppError::Validation("CAS번호가 비어 있습니다.".to_string()));
    }
    let cleaned_name = substance_name.map(str::trim).filter(|s| !s.is_empty());

    if let Some(existing_id) = conn
        .query_row(
            "SELECT id FROM chemical_substances WHERE cas_number = ?1;",
            [normalized_cas.as_str()],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        conn.execute(
            "UPDATE chemical_substances
             SET substance_name = COALESCE(NULLIF(?1, ''), substance_name),
                 updated_at = CASE
                     WHEN NULLIF(?1, '') IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     ELSE updated_at
                 END
             WHERE id = ?2;",
            params![cleaned_name, existing_id],
        )?;
        return Ok(existing_id);
    }

    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES (?1, ?2);",
        params![normalized_cas, cleaned_name],
    )?;
    Ok(conn.last_insert_rowid())
}
