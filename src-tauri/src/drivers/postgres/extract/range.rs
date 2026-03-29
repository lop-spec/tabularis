use serde_json::Value as JsonValue;
use tokio_postgres::types::{Kind, Type};

use crate::drivers::postgres::extract::common::split_at_value_len;

#[inline]
pub fn extract_or_null(ty: &Type, buf: &mut &[u8]) -> JsonValue {
    if buf.len() < 1 {
        return JsonValue::Null;
    };

    let flag = buf[0];
    *buf = &buf[1..];

    if (flag & 1) == 1 {
        return JsonValue::String(String::from("empty"));
    };

    let mut range = String::new();

    range.push(get_lower_bound(flag));

    if flag & (1 << 3) == 0 {
        if let Err(_) = try_extract_bound_into(ty, buf, &mut range) {
            range.push_str("Null, Null");
            range.push(get_upper_bound(flag));
            return JsonValue::String(range);
        };
    };

    range.push_str(", ");

    if flag & (1 << 4) == 0 {
        if let Err(_) = try_extract_bound_into(ty, buf, &mut range) {
            range.push_str("Null");
        };
    }

    range.push(get_upper_bound(flag));

    JsonValue::String(range)
}

#[inline(always)]
fn get_lower_bound(flag: u8) -> char {
    if (flag & (1 << 1)) == 0 {
        '('
    } else {
        '['
    }
}

fn try_extract_bound_into(ty: &Type, buf: &mut &[u8], range: &mut String) -> Result<(), ()> {
    let mut value_buf = match split_at_value_len(buf)? {
        Some(buf) => buf,
        _ => {
            return Err(());
        }
    };

    let val = match ty.kind() {
        Kind::Simple => super::simple::extract_or_null(ty, value_buf),
        Kind::Enum(_variants) => super::r#enum::extract_or_null(&mut value_buf),
        Kind::Array(ty) => super::array::extract_or_null(ty, &mut value_buf),
        Kind::Range(_) => JsonValue::Null, // impossible
        Kind::Domain(ty) => super::simple::extract_or_null(ty, value_buf),
        Kind::Composite(fields) => super::composite::extract_or_null(fields, &mut value_buf),
        _ => JsonValue::Null, // unsupported
    };

    range.push_str(&val.to_string());
    Ok(())
}

#[inline(always)]
fn get_upper_bound(flag: u8) -> char {
    if (flag & (1 << 2)) == 0 {
        ')'
    } else {
        ']'
    }
}
