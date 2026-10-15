//! Shared geographic coordinate parsing and validation for input and decoded data.
mod display;
mod input;
pub use display::{CoordinateDisplay, CoordinateFormat};
pub use input::{CoordinateInputError, parse_coordinate_query};
#[derive(Debug, PartialEq, Eq)]
pub enum CoordinateError {
    Latitude,
    Longitude,
}
impl std::fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Latitude => "Latitude must be a finite number from -90 to 90 degrees.",
            Self::Longitude => "Longitude must be a finite number from -180 to 180 degrees.",
        })
    }
}
impl std::error::Error for CoordinateError {}

/// Parse a latitude/longitude query separated by commas, semicolons or whitespace.
/// Commas separate coordinates here; decimal commas are supported only by the
/// separate-field parser below, where they are unambiguous.
pub fn parse_latlon(query: &str) -> Option<(f64, f64)> {
    parse_coordinate_query(query).ok().flatten()
}
fn parse_decimal_pair(query: &str) -> Option<(f64, f64)> {
    let cleaned = query.replace([',', ';'], " ");
    let mut parts = cleaned.split_whitespace();
    let latitude = parts.next()?;
    let longitude = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    parse_coordinates(latitude, longitude).ok()
}

/// Parse separate latitude and longitude fields, accepting decimal commas and
/// optional degree symbols. Bounds are inclusive; non-finite values are rejected.
pub fn parse_coordinates(latitude: &str, longitude: &str) -> Result<(f64, f64), CoordinateError> {
    fn parse(text: &str) -> Option<f64> {
        text.trim()
            .trim_end_matches('°')
            .trim()
            .replace(',', ".")
            .parse()
            .ok()
    }
    validate_coordinates(
        parse(latitude).ok_or(CoordinateError::Latitude)?,
        parse(longitude).ok_or(CoordinateError::Longitude)?,
    )
}

/// Validate already-decoded latitude/longitude using the same rules as text input.
/// Returned values preserve f64 precision and normalize negative zero.
pub fn validate_coordinates(latitude: f64, longitude: f64) -> Result<(f64, f64), CoordinateError> {
    const MAX_LATITUDE_DEG: f64 = 90.0;
    const MAX_LONGITUDE_DEG: f64 = 180.0;
    if !latitude.is_finite() || latitude.abs() > MAX_LATITUDE_DEG {
        return Err(CoordinateError::Latitude);
    }
    if !longitude.is_finite() || longitude.abs() > MAX_LONGITUDE_DEG {
        return Err(CoordinateError::Longitude);
    }
    Ok((
        if latitude == 0.0 { 0.0 } else { latitude },
        if longitude == 0.0 { 0.0 } else { longitude },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_validation_matches_text_rules_without_rounding() {
        for (latitude, longitude) in [
            (90.0, 180.0),
            (-90.0, -180.0),
            (12.3456789012345, -98.7654321098765),
        ] {
            assert_eq!(
                validate_coordinates(latitude, longitude),
                Ok((latitude, longitude))
            );
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 90.00001, -91.0] {
            assert_eq!(
                validate_coordinates(bad, 0.0),
                Err(CoordinateError::Latitude)
            );
        }
        for bad in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            180.00001,
            -181.0,
        ] {
            assert_eq!(
                validate_coordinates(0.0, bad),
                Err(CoordinateError::Longitude)
            );
        }
        let zero = validate_coordinates(-0.0, -0.0).unwrap();
        assert!(!zero.0.is_sign_negative() && !zero.1.is_sign_negative());
    }
    #[test]
    fn exact_input_retains_f64_precision_including_boundaries() {
        assert_eq!(parse_coordinates("90", "180"), Ok((90.0, 180.0)));
        assert_eq!(parse_coordinates("-90°", "-180°"), Ok((-90.0, -180.0)));
        assert_eq!(
            parse_coordinates("12,3456789012345", "-98.7654321098765"),
            Ok((12.3456789012345, -98.7654321098765))
        );
        for value in [0.0000000000001, 89.99999999999999, -45.123456789012345] {
            assert_eq!(parse_coordinates(&value.to_string(), "0").unwrap().0, value);
        }
        assert!(!parse_coordinates("-0", "0").unwrap().0.is_sign_negative());
    }
    #[test]
    fn invalid_nonfinite_and_out_of_range_coordinates_are_rejected() {
        for value in [
            "NaN",
            "inf",
            "-inf",
            "1e999",
            "90.0000000001",
            "-90.1",
            "",
            "1,2,3",
        ] {
            assert_eq!(
                parse_coordinates(value, "0"),
                Err(CoordinateError::Latitude)
            );
        }
        for value in ["NaN", "inf", "180.00000001", "-181", ""] {
            assert_eq!(
                parse_coordinates("0", value),
                Err(CoordinateError::Longitude)
            );
        }
    }
    #[test]
    fn coordinates_accept_existing_separators_and_bounds() {
        for input in ["-15, -50", "-15; -50", " -15  -50 ", "-15°, -50°"] {
            assert_eq!(parse_latlon(input), Some((-15.0, -50.0)));
        }
        assert_eq!(parse_latlon("90, 180"), Some((90.0, 180.0)));
        for input in [
            "91, 0", "0, 181", "NaN, 0", "inf, 0", "one, two", "1, 2, 3", "", "1",
        ] {
            assert_eq!(parse_latlon(input), None, "{input}");
        }
    }
}
