use std::cmp::Ordering;
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::str::FromStr;

const MAX_SCALE: u32 = 28;
const MAX_I128_POW10: usize = 38;

fn pow10_u128(exp: u32) -> Option<u128> {
    if exp as usize > MAX_I128_POW10 {
        return None;
    }
    10u128.checked_pow(exp)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingStrategy {
    MidpointAwayFromZero,
    MidpointNearestEven,
    ToZero,
    AwayFromZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

/// Lightweight 128-bit fixed-point decimal compatible with `rust_decimal` APIs
/// required for exact chemical weight and percentage calculations.
#[derive(Clone, Copy, Eq)]
pub struct Decimal {
    mantissa: i128,
    scale: u32,
}

impl Decimal {
    pub const ZERO: Self = Self {
        mantissa: 0,
        scale: 0,
    };
    pub const ONE: Self = Self {
        mantissa: 1,
        scale: 0,
    };
    pub const TEN: Self = Self {
        mantissa: 10,
        scale: 0,
    };
    pub const ONE_HUNDRED: Self = Self {
        mantissa: 100,
        scale: 0,
    };
    pub const ONE_THOUSAND: Self = Self {
        mantissa: 1000,
        scale: 0,
    };

    pub const fn new(num: i64, scale: u32) -> Self {
        assert!(scale <= MAX_SCALE, "scale exceeds maximum of 28");
        Self {
            mantissa: num as i128,
            scale,
        }
    }

    pub const fn from_i128_with_scale(num: i128, scale: u32) -> Self {
        assert!(scale <= MAX_SCALE, "scale exceeds maximum of 28");
        Self {
            mantissa: num,
            scale,
        }
    }

    pub const fn mantissa(&self) -> i128 {
        self.mantissa
    }

    pub const fn scale(&self) -> u32 {
        self.scale
    }

    pub const fn is_zero(&self) -> bool {
        self.mantissa == 0
    }

    pub const fn is_sign_negative(&self) -> bool {
        self.mantissa < 0
    }

    pub const fn is_sign_positive(&self) -> bool {
        self.mantissa > 0
    }

    pub const fn abs(&self) -> Self {
        Self {
            mantissa: self.mantissa.abs(),
            scale: self.scale,
        }
    }

    pub fn from_str_exact(s: &str) -> Result<Self, Error> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(Error("empty decimal string".to_string()));
        }

        let (neg, rest) = if let Some(r) = trimmed.strip_prefix('-') {
            (true, r)
        } else if let Some(r) = trimmed.strip_prefix('+') {
            (false, r)
        } else {
            (false, trimmed)
        };

        if rest.is_empty() {
            return Err(Error(format!("invalid decimal string: {s}")));
        }

        let (int_part, frac_part) = match rest.split_once('.') {
            Some((i, f)) => {
                if f.contains('.') {
                    return Err(Error(format!("multiple decimal points in: {s}")));
                }
                if i.is_empty() && f.is_empty() {
                    return Err(Error(format!("invalid decimal string: {s}")));
                }
                if !i.bytes().all(|b| b.is_ascii_digit()) || !f.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(Error(format!("non-digit characters in: {s}")));
                }
                (i, f)
            }
            None => {
                if !rest.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(Error(format!("non-digit characters in: {s}")));
                }
                (rest, "")
            }
        };

        let scale = u32::try_from(frac_part.len())
            .map_err(|_| Error("fractional length overflow".to_string()))?;
        if scale > MAX_SCALE {
            return Err(Error(format!("scale {scale} exceeds maximum {MAX_SCALE}")));
        }

        let mut combined = String::with_capacity(int_part.len() + frac_part.len());
        combined.push_str(if int_part.is_empty() { "0" } else { int_part });
        combined.push_str(frac_part);

        let unsigned: u128 = combined
            .parse()
            .map_err(|_| Error(format!("decimal magnitude overflow: {s}")))?;
        let mantissa = if neg {
            if unsigned == 0 {
                0
            } else {
                let signed = i128::try_from(unsigned)
                    .map_err(|_| Error(format!("decimal magnitude overflow: {s}")))?;
                -signed
            }
        } else {
            i128::try_from(unsigned)
                .map_err(|_| Error(format!("decimal magnitude overflow: {s}")))?
        };

        Ok(Self { mantissa, scale })
    }

    pub fn normalize(&self) -> Self {
        if self.mantissa == 0 {
            return Self::ZERO;
        }
        let mut m = self.mantissa;
        let mut s = self.scale;
        while s > 0 && m % 10 == 0 {
            m /= 10;
            s -= 1;
        }
        Self {
            mantissa: m,
            scale: s,
        }
    }

    pub fn round_dp(&self, dp: u32) -> Self {
        self.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven)
    }

    pub fn round_dp_with_strategy(&self, dp: u32, strategy: RoundingStrategy) -> Self {
        if self.scale <= dp || self.mantissa == 0 {
            return *self;
        }
        let diff = self.scale - dp;
        let Some(divisor) = pow10_u128(diff) else {
            return Self {
                mantissa: 0,
                scale: dp.min(MAX_SCALE),
            };
        };

        let neg = self.mantissa < 0;
        let mag = self.mantissa.unsigned_abs();
        let quotient = mag / divisor;
        let rem = mag % divisor;
        let half = divisor / 2;

        let round_up = match strategy {
            RoundingStrategy::ToZero => false,
            RoundingStrategy::AwayFromZero => rem > 0,
            RoundingStrategy::MidpointAwayFromZero => rem >= half,
            RoundingStrategy::MidpointNearestEven => {
                if rem > half {
                    true
                } else if rem < half {
                    false
                } else {
                    quotient % 2 == 1
                }
            }
        };

        let rounded_mag = if round_up { quotient + 1 } else { quotient };
        let rounded = rounded_mag as i128;
        Self {
            mantissa: if neg && rounded != 0 {
                -rounded
            } else {
                rounded
            },
            scale: dp,
        }
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        let target_scale = self.scale.max(other.scale);
        let lhs = self.rescale_exact(target_scale)?;
        let rhs = other.rescale_exact(target_scale)?;
        let sum = lhs.checked_add(rhs)?;
        Some(Self {
            mantissa: sum,
            scale: target_scale,
        })
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        let target_scale = self.scale.max(other.scale);
        let lhs = self.rescale_exact(target_scale)?;
        let rhs = other.rescale_exact(target_scale)?;
        let diff = lhs.checked_sub(rhs)?;
        Some(Self {
            mantissa: diff,
            scale: target_scale,
        })
    }

    pub fn checked_mul(self, other: Self) -> Option<Self> {
        if self.mantissa == 0 || other.mantissa == 0 {
            return Some(Self::ZERO);
        }
        let neg = (self.mantissa < 0) ^ (other.mantissa < 0);
        let prod = mul_u128_wide(self.mantissa.unsigned_abs(), other.mantissa.unsigned_abs());
        let mut scale = self.scale + other.scale;

        let mut val = prod;
        while scale > MAX_SCALE {
            let (q, r) = div_u256_by_u128(val, 10);
            if scale == MAX_SCALE + 1 && r >= 5 {
                val = add_u256_u128(q, 1);
            } else {
                val = q;
            }
            scale -= 1;
        }
        while val.1 > 0 && scale > 0 {
            let (q, r) = div_u256_by_u128(val, 10);
            val = if r >= 5 { add_u256_u128(q, 1) } else { q };
            scale -= 1;
        }
        if val.1 > 0 {
            return None;
        }
        let mag = i128::try_from(val.0).ok()?;
        Some(Self {
            mantissa: if neg && mag != 0 { -mag } else { mag },
            scale,
        })
    }

    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.mantissa == 0 {
            return None;
        }
        if self.mantissa == 0 {
            return Some(Self::ZERO);
        }
        let neg = (self.mantissa < 0) ^ (other.mantissa < 0);
        let lhs = self.mantissa.unsigned_abs();
        let rhs = other.mantissa.unsigned_abs();

        // Integer quotient + exact long-division fractional digits up to MAX_SCALE
        let mut q = lhs / rhs;
        let mut rem = lhs % rhs;

        // Adjust initial scale difference: result = (lhs / rhs) * 10^(other.scale - self.scale)
        // Compute fractional digits up to 28 total scale.
        let mut extra_scale: i32 = (self.scale as i32) - (other.scale as i32);
        let mut digits = Vec::new();
        while extra_scale < 0 {
            let prod = rem * 10;
            let digit = prod / rhs;
            rem = prod % rhs;
            q = q.checked_mul(10)?.checked_add(digit)?;
            extra_scale += 1;
        }
        let mut target_scale = extra_scale as u32;
        if target_scale > MAX_SCALE {
            // Divide down to MAX_SCALE if needed
            while target_scale > MAX_SCALE {
                q /= 10;
                target_scale -= 1;
            }
        } else {
            while target_scale < MAX_SCALE && rem != 0 {
                let prod = mul_u128_wide(rem, 10);
                let (digit, next_rem) = div_u256_by_u128_wide(prod, rhs);
                if q.checked_mul(10).and_then(|v| v.checked_add(digit)).is_none() {
                    break;
                }
                digits.push(digit);
                q = q * 10 + digit;
                rem = next_rem;
                target_scale += 1;
            }
        }
        let _ = digits;

        let mag = i128::try_from(q).ok()?;
        Some(
            Self {
                mantissa: if neg && mag != 0 { -mag } else { mag },
                scale: target_scale,
            }
            .normalize(),
        )
    }

    pub fn to_f64(&self) -> Option<f64> {
        self.to_string().parse::<f64>().ok()
    }

    fn rescale_exact(self, target_scale: u32) -> Option<i128> {
        if self.scale == target_scale {
            return Some(self.mantissa);
        }
        let diff = target_scale - self.scale;
        let factor = pow10_u128(diff)?;
        let factor_i128 = i128::try_from(factor).ok()?;
        self.mantissa.checked_mul(factor_i128)
    }
}

/// 256-bit unsigned integer (low, high) for intermediate multiplication/division.
fn mul_u128_wide(a: u128, b: u128) -> (u128, u128) {
    let a_lo = a & 0xFFFF_FFFF_FFFF_FFFF;
    let a_hi = a >> 64;
    let b_lo = b & 0xFFFF_FFFF_FFFF_FFFF;
    let b_hi = b >> 64;

    let lo_lo = a_lo * b_lo;
    let hi_lo = a_hi * b_lo;
    let lo_hi = a_lo * b_hi;
    let hi_hi = a_hi * b_hi;

    let cross = (lo_lo >> 64) + (hi_lo & 0xFFFF_FFFF_FFFF_FFFF) + (lo_hi & 0xFFFF_FFFF_FFFF_FFFF);
    let low = (lo_lo & 0xFFFF_FFFF_FFFF_FFFF) | ((cross & 0xFFFF_FFFF_FFFF_FFFF) << 64);
    let high = hi_hi + (hi_lo >> 64) + (lo_hi >> 64) + (cross >> 64);
    (low, high)
}

fn add_u256_u128(val: (u128, u128), add: u128) -> (u128, u128) {
    let (low, overflow) = val.0.overflowing_add(add);
    (low, val.1 + u128::from(overflow))
}

fn div_u256_by_u128(val: (u128, u128), divisor: u128) -> ((u128, u128), u128) {
    let (q_hi, r_hi) = (val.1 / divisor, val.1 % divisor);
    let (q_lo, r_lo) = div_u256_by_u128_wide((val.0, r_hi), divisor);
    ((q_lo, q_hi), r_lo)
}

fn div_u256_by_u128_wide(val: (u128, u128), divisor: u128) -> (u128, u128) {
    // Bitwise long division when high < divisor
    let mut rem = val.1;
    let mut q = 0u128;
    for shift in (0..128).rev() {
        let bit = (val.0 >> shift) & 1;
        let carry = rem >> 127;
        rem = (rem << 1) | bit;
        if carry != 0 || rem >= divisor {
            rem = rem.wrapping_sub(divisor);
            q |= 1u128 << shift;
        }
    }
    (q, rem)
}

impl FromStr for Decimal {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_exact(s)
    }
}

impl fmt::Debug for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mantissa == 0 {
            if self.scale == 0 {
                return f.write_str("0");
            }
            let zeros = "0".repeat(self.scale as usize);
            return write!(f, "0.{zeros}");
        }

        let neg = self.mantissa < 0;
        let digits = self.mantissa.unsigned_abs().to_string();
        if neg {
            f.write_str("-")?;
        }

        let scale = self.scale as usize;
        if scale == 0 {
            return f.write_str(&digits);
        }

        if digits.len() <= scale {
            let pad = "0".repeat(scale - digits.len());
            write!(f, "0.{pad}{digits}")
        } else {
            let split = digits.len() - scale;
            write!(f, "{}.{}", &digits[..split], &digits[split..])
        }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        let a = self.normalize();
        let b = other.normalize();
        a.mantissa == b.mantissa && a.scale == b.scale
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.normalize();
        let b = other.normalize();
        let target_scale = a.scale.max(b.scale);
        if let (Some(la), Some(lb)) = (
            a.rescale_exact(target_scale),
            b.rescale_exact(target_scale),
        ) {
            la.cmp(&lb)
        } else {
            // Fallback using sign + integer/fraction comparison
            match (a.mantissa < 0, b.mantissa < 0) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => {
                    let cmp_abs = compare_abs_decimal(a, b);
                    if a.mantissa < 0 {
                        cmp_abs.reverse()
                    } else {
                        cmp_abs
                    }
                }
            }
        }
    }
}

fn compare_abs_decimal(a: Decimal, b: Decimal) -> Ordering {
    let sa = a.abs().to_string();
    let sb = b.abs().to_string();
    let (ai, af) = sa.split_once('.').unwrap_or((&sa, ""));
    let (bi, bf) = sb.split_once('.').unwrap_or((&sb, ""));
    match ai.len().cmp(&bi.len()).then_with(|| ai.cmp(bi)) {
        Ordering::Equal => {
            let max_f = af.len().max(bf.len());
            for idx in 0..max_f {
                let ac = af.as_bytes().get(idx).copied().unwrap_or(b'0');
                let bc = bf.as_bytes().get(idx).copied().unwrap_or(b'0');
                match ac.cmp(&bc) {
                    Ordering::Equal => continue,
                    non_eq => return non_eq,
                }
            }
            Ordering::Equal
        }
        non_eq => non_eq,
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Neg for Decimal {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            mantissa: -self.mantissa,
            scale: self.scale,
        }
    }
}

impl Add for Decimal {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).expect("decimal addition overflow")
    }
}

impl AddAssign for Decimal {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Decimal {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs).expect("decimal subtraction overflow")
    }
}

impl SubAssign for Decimal {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for Decimal {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(rhs)
            .expect("decimal multiplication overflow")
    }
}

impl Div for Decimal {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        self.checked_div(rhs).expect("decimal division error")
    }
}

impl Sum for Decimal {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + x)
    }
}
