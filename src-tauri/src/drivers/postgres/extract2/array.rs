use serde_json::Value as JsonValue;
use tokio_postgres::types::{Kind, Type};

use super::common::split_at_value_len;

pub fn extract_or_null(ty: &Type, buf: &mut &[u8]) -> Result<JsonValue, ()> {
    // array must be at least 12 bytes (header) except if it is `NULL`
    if buf.len() == 0 {
        return Ok(JsonValue::Null);
    };

    if buf.len() < 12 {
        log::error!("array buffer too short: {}", buf.len());
        return Err(());
    };

    let dimentions = i32::from_be_bytes(buf[..4].try_into().unwrap());

    // i don't think this is possible but just in case
    if dimentions < 1 {
        log::error!("invalid number of dimensions: {}", dimentions);
        return Err(());
    };

    // max dimensions is 64 and just for safety
    if dimentions > 64 {
        log::error!("too many dimensions: {}", dimentions);
        return Err(());
    }

    let dimentions = dimentions as usize;

    // each dimension must have at least 8 bytes info
    if buf.len() * dimentions < 8 * dimentions {
        log::error!("array buffer too short: {}", buf.len());
        return Err(());
    };

    // ignore `has nulls` 4 bytes
    // ignore `element type` 4 bytes because we already have it
    *buf = &buf[12..];

    let mut total_vecs: usize = 1;
    let mut arr_lengths = Vec::with_capacity(dimentions);

    for i in 0..dimentions {
        let length = i32::from_be_bytes(buf[..4].try_into().unwrap());

        // i don't think this is possible but just in case
        if length < 0 {
            log::error!("invalid length: {}", length);
            return Err(());
        };

        let length = length as usize;

        arr_lengths.push(length);

        *buf = &buf[8..]; // skip `lower bound` 4 bytes

        if dimentions - i == 1 {
            continue;
        };

        let all_vecs_in_this_lvl = match total_vecs.checked_mul(length) {
            Some(v) => v,
            None => {
                log::error!("overflow: total_vecs={} length={}", total_vecs, length);
                return Err(());
            }
        };

        total_vecs = match total_vecs.checked_add(all_vecs_in_this_lvl) {
            Some(v) => v,
            None => {
                log::error!(
                    "overflow: total_vecs={} all_vecs_in_this_lvl={}",
                    total_vecs,
                    all_vecs_in_this_lvl
                );
                return Err(());
            }
        };
    }

    // SAFETY: i think this number should be discussed
    if total_vecs > 1024 {
        log::error!("too many vectors: total_vecs={}", total_vecs);
        return Err(());
    };

    let mut vec = Vec::with_capacity(arr_lengths[0]);

    extract_recursively_into(&mut vec, &arr_lengths, 1, &ty, buf)?;

    Ok(JsonValue::Array(vec))
}

fn extract_recursively_into(
    vec: &mut Vec<JsonValue>,
    arr_lengths: &[usize],
    depth: usize,
    ty: &Type,
    buf: &mut &[u8],
) -> Result<(), ()> {
    match depth == arr_lengths.len() {
        true => {
            for _ in 0..arr_lengths[depth - 1] {
                vec.push(extract_elem(ty, buf)?);
            }
        }

        false => {
            for _ in 0..arr_lengths[depth - 1] {
                let mut sub_vec = Vec::with_capacity(arr_lengths[depth - 1]);
                extract_recursively_into(&mut sub_vec, arr_lengths, depth + 1, ty, buf)?;
                vec.push(JsonValue::Array(sub_vec));
            }
        }
    };

    Ok(())
}

#[inline]
fn extract_elem(ty: &Type, buf: &mut &[u8]) -> Result<JsonValue, ()> {
    let mut value_buf = split_at_value_len(buf)?;

    match ty.kind() {
        Kind::Simple => Ok(super::simple::extract_or_null(ty, value_buf)),
        Kind::Domain(inner) => Ok(super::simple::extract_or_null(inner, value_buf)),
        Kind::Array(_) => Ok(JsonValue::Null), // impossible case
        Kind::Composite(fields) => super::composite::extract_or_null(fields, &mut value_buf),
        _ => Ok(JsonValue::Null),
    }
}
