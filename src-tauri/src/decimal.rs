use crate::error::{AppError, AppResult};
use rust_decimal::{Decimal, RoundingStrategy};
use std::str::FromStr;

pub const SCALE_FACTOR: i64 = 10_000;
pub const HUNDRED_PERCENT_SCALED: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDecimal4dp {
    pub decimal: Decimal,
    pub canonical_text: String,
    pub scaled_i64: i64,
}

/// Losslessly parses a decimal string (up to 4 decimal places) into a
/// canonical normalized `Decimal` string and `× 10,000` scaled `i64`.
pub fn parse_decimal_4dp(input: &str) -> AppResult<ParsedDecimal4dp> {
    let cleaned = input.trim().replace(',', "");
    if cleaned.is_empty() {
        return Err(AppError::Validation("숫자 값이 비어 있습니다.".to_string()));
    }
    let parsed = Decimal::from_str(&cleaned)
        .map_err(|_| AppError::Validation(format!("올바른 숫자 형식이 아닙니다: '{input}'")))?;
    let normalized = parsed.normalize();
    let scaled_i64 = to_scaled_4dp(normalized)?;
    Ok(ParsedDecimal4dp {
        decimal: normalized,
        canonical_text: normalized.to_string(),
        scaled_i64,
    })
}

/// Converts a `Decimal` with up to 4 decimal places into a `× 10,000` scaled `i64`.
pub fn to_scaled_4dp(value: Decimal) -> AppResult<i64> {
    let norm = value.normalize();
    if norm.scale() > 4 {
        return Err(AppError::Validation(format!(
            "소수점 4자리 이하만 입력할 수 있습니다: '{value}'"
        )));
    }
    let shift = 4 - norm.scale();
    let factor = 10i128.pow(shift);
    let scaled = norm
        .mantissa()
        .checked_mul(factor)
        .ok_or_else(|| AppError::Validation("수량 범위가 허용치를 초과했습니다.".to_string()))?;
    i64::try_from(scaled)
        .map_err(|_| AppError::Validation("수량 범위가 허용치를 초과했습니다.".to_string()))
}

pub fn from_scaled_4dp(scaled: i64) -> Decimal {
    Decimal::from_i128_with_scale(scaled as i128, 4).normalize()
}

/// Exact fixed-point calculation: `import_weight_kg * (weight_pct / 100)`.
pub fn compute_substance_weight_kg(import_weight_kg: Decimal, weight_pct: Decimal) -> Decimal {
    (import_weight_kg * weight_pct) / Decimal::ONE_HUNDRED
}

/// Formats `함유율 (%)` with minimum 2 decimals and up to 4 significant decimals.
pub fn format_percentage(value: Decimal) -> String {
    format_decimal(value, 2, 4, false)
}

/// Formats `kg` with comma grouping, minimum 2 decimals and up to 4 significant decimals.
pub fn format_kg(value: Decimal) -> String {
    format_decimal(value, 2, 4, true)
}

/// Formats `ton` (`= kg / 1,000`) with comma grouping and 4 decimals,
/// expanding to 5 decimals if rounding to 4 decimals would hide a small non-zero value.
pub fn format_ton(value: Decimal) -> String {
    let rounded_4 = value.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
    let max_dp = if !value.is_zero() && rounded_4.is_zero() {
        5
    } else {
        4
    };
    format_decimal(value, 4, max_dp, true)
}

fn format_decimal(value: Decimal, min_dp: u32, max_dp: u32, group_thousands: bool) -> String {
    let rounded = value
        .round_dp_with_strategy(max_dp, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    let neg = rounded.is_sign_negative();
    let abs_str = rounded.abs().to_string();
    let (int_part, frac_part) = abs_str.split_once('.').unwrap_or((&abs_str, ""));

    let formatted_int = if group_thousands {
        insert_thousands_commas(int_part)
    } else {
        int_part.to_string()
    };

    let mut frac = frac_part.to_string();
    while frac.len() < min_dp as usize {
        frac.push('0');
    }

    let sign = if neg && (formatted_int != "0" || frac.bytes().any(|b| b != b'0')) {
        "-"
    } else {
        ""
    };

    if frac.is_empty() {
        format!("{sign}{formatted_int}")
    } else {
        format!("{sign}{formatted_int}.{frac}")
    }
}

fn insert_thousands_commas(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
