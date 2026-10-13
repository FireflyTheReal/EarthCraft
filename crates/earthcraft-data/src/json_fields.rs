//! Decode valid fields while ignoring ill-typed siblings in partial responses.
use serde::{Deserialize, Deserializer};
use serde_json::Value;
pub(crate) fn string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Value::deserialize(d)?.as_str().map(str::to_owned))
}
pub(crate) fn number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(Value::deserialize(d)?.as_f64())
}
pub(crate) fn string_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(Value::deserialize(d)?.as_str().and_then(|s| s.parse().ok()))
}
pub(crate) fn string_numbers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
    Ok(Value::deserialize(d)?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str()?.parse().ok())
        .collect())
}

pub(crate) fn boolean<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<bool>, D::Error> {
    let value = serde_json::Value::deserialize(decoder)?;
    Ok(value.as_bool())
}
