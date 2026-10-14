//! Decode valid fields while ignoring ill-typed siblings in partial responses.
use serde::{Deserialize, Deserializer};
use serde_json::Value;
pub(crate) fn string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Value::deserialize(d)?.as_str().map(str::to_owned))
}
pub(crate) fn number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(Value::deserialize(d)?.as_f64())
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

/// Decode GeoJSON/Photon's longitude-first position without changing domain order.
pub(crate) fn coordinates(value: &Value) -> Option<(f64, f64)> {
    earthcraft_core::coordinates::validate_coordinates(
        value.get(1)?.as_f64()?,
        value.get(0)?.as_f64()?,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoded_positions_validate_bounds_order_and_optional_altitude() {
        assert_eq!(
            coordinates(&serde_json::json!([-180, 90, 123])),
            Some((90.0, -180.0))
        );
        for value in [
            serde_json::json!([0, 91]),
            serde_json::json!([181, 0]),
            serde_json::json!([0]),
            serde_json::json!(["10", "20"]),
            serde_json::Value::Null,
        ] {
            assert_eq!(coordinates(&value), None);
        }
    }
}
