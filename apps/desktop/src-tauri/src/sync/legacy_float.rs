//! Upgrade bridge for the `serde_json` `float_roundtrip` switch (#1435).
//!
//! Before it, a float read from JSON text went through `serde_json`'s fast
//! decimal→f64 path, which can be one ulp off. A sync baseline stores only the
//! record hash, so a payload that was parsed by the old path hashes differently
//! under the exact parser. [`legacy_record_hash`] recomputes the hash the old
//! parser would have produced so the engine can tell "same record, new parser"
//! from a genuine local edit.
//!
//! The converter is a port of `serde_json` 1.0.154 `de.rs`, the
//! `#[cfg(not(feature = "float_roundtrip"))]` path: `parse_integer`,
//! `parse_decimal`, `parse_exponent`, `parse_long_integer`,
//! `parse_decimal_overflow`, `parse_exponent_overflow` and `f64_from_parts`
//! (the `POW10` multiply/divide). Delete this module once no pre-switch
//! baselines can remain.

use serde_json::{Number, Value};

use super::envelope::record_hash;

/// Hash of `payload` as the pre-`float_roundtrip` build would have computed it.
pub fn legacy_record_hash(payload: &Value) -> String {
    record_hash(&rewrite(payload))
}

fn rewrite(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => serde_json::to_string(n)
            .ok()
            .and_then(|text| legacy_parse_f64(&text))
            .and_then(Number::from_f64)
            .map_or_else(|| v.clone(), Value::Number),
        Value::Array(a) => Value::Array(a.iter().map(rewrite).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), rewrite(v))).collect()),
        other => other.clone(),
    }
}

/// `u64::MAX`-aware `significand * 10 + digit` overflow test (`overflow!`).
fn overflows(significand: u64, digit: u64) -> bool {
    let max = u64::MAX;
    significand >= max / 10 && (significand > max / 10 || digit > max % 10)
}

/// A JSON number token as the old parser read it; `None` where it errored
/// (out of range) or the text is not a number.
pub(crate) fn legacy_parse_f64(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let (positive, mut i) = match b.first()? {
        b'-' => (false, 1),
        _ => (true, 0),
    };
    // parse_integer
    let first = *b.get(i)?;
    if !first.is_ascii_digit() {
        return None;
    }
    let mut sig = u64::from(first - b'0');
    i += 1;
    let mut exp = 0i32;
    let mut overflowed = false;
    if first != b'0' {
        while let Some(&c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            let d = u64::from(c - b'0');
            if overflows(sig, d) {
                overflowed = true;
                break;
            }
            sig = sig * 10 + d;
            i += 1;
        }
    }
    if overflowed {
        // parse_long_integer: remaining integer digits only scale the exponent.
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            exp += 1;
        }
    }
    // parse_decimal (also reached from parse_long_integer)
    if b.get(i) == Some(&b'.') {
        i += 1;
        let mut frac = 0i32;
        while let Some(&c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            let d = u64::from(c - b'0');
            if overflows(sig, d) {
                // parse_decimal_overflow: ignore all further digits.
                while b.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
                break;
            }
            sig = sig * 10 + d;
            frac -= 1;
            i += 1;
        }
        exp += frac;
    } else if !overflowed && !matches!(b.get(i), Some(b'e' | b'E')) {
        return None; // an integer, not a float token
    }
    // parse_exponent
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        let pos_exp = match b.get(i) {
            Some(b'+') => {
                i += 1;
                true
            }
            Some(b'-') => {
                i += 1;
                false
            }
            _ => true,
        };
        let mut e = i32::from(b.get(i).filter(|c| c.is_ascii_digit())? - b'0');
        i += 1;
        while let Some(&c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            i += 1;
            let d = i32::from(c - b'0');
            if e >= i32::MAX / 10 && (e > i32::MAX / 10 || d > i32::MAX % 10) {
                // parse_exponent_overflow
                if sig != 0 && pos_exp {
                    return None;
                }
                return Some(if positive { 0.0 } else { -0.0 });
            }
            e = e * 10 + d;
        }
        exp = if pos_exp {
            exp.saturating_add(e)
        } else {
            exp.saturating_sub(e)
        };
    }
    f64_from_parts(positive, sig, exp)
}

/// `POW10[i]`: the correctly rounded `1e<i>` (what the literal table holds).
fn pow10(i: usize) -> Option<f64> {
    (i <= 308).then(|| format!("1e{i}").parse().unwrap_or(f64::INFINITY))
}

/// `f64_from_parts` without `float_roundtrip`.
#[allow(clippy::cast_precision_loss)]
fn f64_from_parts(positive: bool, significand: u64, mut exponent: i32) -> Option<f64> {
    let mut f = significand as f64;
    loop {
        match pow10(exponent.wrapping_abs() as usize) {
            Some(pow) => {
                if exponent >= 0 {
                    f *= pow;
                    if f.is_infinite() {
                        return None;
                    }
                } else {
                    f /= pow;
                }
                break;
            }
            None => {
                if f == 0.0 {
                    break;
                }
                if exponent >= 0 {
                    return None;
                }
                f /= 1e308;
                exponent += 308;
            }
        }
    }
    Some(if positive { f } else { -f })
}
