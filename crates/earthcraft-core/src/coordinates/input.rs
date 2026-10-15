//! Recognize coordinates before sending place-name searches to a remote provider.
use super::{CoordinateError, parse_decimal_pair, validate_coordinates};
use mgrs_convert::{Mgrs, UtmUps};

#[derive(Debug)]
pub enum CoordinateInputError {
    Coordinate(CoordinateError),
    Grid(mgrs_convert::Error),
    Format(&'static str),
}
impl std::fmt::Display for CoordinateInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coordinate(error) => error.fmt(f),
            Self::Grid(error) => error.fmt(f),
            Self::Format(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for CoordinateInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Grid(error) => Some(error),
            Self::Format(_) => None,
        }
    }
}
impl From<CoordinateError> for CoordinateInputError {
    fn from(error: CoordinateError) -> Self {
        Self::Coordinate(error)
    }
}
impl From<mgrs_convert::Error> for CoordinateInputError {
    fn from(error: mgrs_convert::Error) -> Self {
        Self::Grid(error)
    }
}
const ANGLE_INPUT_HELP: &str = "Enter latitude then longitude in decimal degrees, DMS or decimal minutes; minutes and seconds must be less than 60.";
const GRID_INPUT_HELP: &str = "Enter UTM as zone N/S easting northing, UPS as UPS N/S easting northing, or a complete MGRS grid reference.";

pub fn parse_coordinate_query(query: &str) -> Result<Option<(f64, f64)>, CoordinateInputError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }
    if let Some(position) = parse_decimal_pair(query) {
        return Ok(Some(position));
    }
    let upper = query.to_uppercase();
    let compact: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
    let zone_digits = compact.bytes().take_while(u8::is_ascii_digit).count();
    let letters = compact[zone_digits..]
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    let tail = &compact[zone_digits + letters..];
    let mgrs_shape = compact.is_ascii()
        && zone_digits <= 2
        && letters == 3
        && !tail.is_empty()
        && tail.bytes().all(|b| b.is_ascii_digit());
    if mgrs_shape {
        let point = compact.parse::<Mgrs>()?.to_latlon();
        return Ok(Some(validate_coordinates(point.lat(), point.lon())?));
    }
    let mut tokens: Vec<_> = upper.split_whitespace().collect();
    if tokens.len() == 3 && tokens[0].len() >= 2 && tokens[0].is_ascii() {
        let (zone, hemisphere) = tokens[0].split_at(tokens[0].len() - 1);
        if zone.bytes().all(|b| b.is_ascii_digit()) && matches!(hemisphere, "N" | "S") {
            tokens = vec![zone, hemisphere, tokens[1], tokens[2]];
        }
    }
    if tokens.first() == Some(&"UPS")
        || (tokens.len() == 4
            && tokens[0].bytes().all(|b| b.is_ascii_digit())
            && matches!(tokens[1], "N" | "S"))
    {
        if tokens.len() != 4 || !matches!(tokens[1], "N" | "S") {
            return Err(CoordinateInputError::Format(GRID_INPUT_HELP));
        }
        let zone = if tokens[0] == "UPS" {
            0
        } else {
            tokens[0]
                .parse()
                .map_err(|_| CoordinateInputError::Format(GRID_INPUT_HELP))?
        };
        let easting: f64 = tokens[2]
            .parse()
            .map_err(|_| CoordinateInputError::Format(GRID_INPUT_HELP))?;
        let northing: f64 = tokens[3]
            .parse()
            .map_err(|_| CoordinateInputError::Format(GRID_INPUT_HELP))?;
        if !easting.is_finite() || !northing.is_finite() {
            return Err(CoordinateInputError::Format(GRID_INPUT_HELP));
        }
        let grid = if zone == 0 {
            UtmUps::ups(tokens[1] == "N", easting, northing)?
        } else {
            UtmUps::utm(zone, tokens[1] == "N", easting, northing)?
        };
        let point = grid.to_latlon();
        return Ok(Some(validate_coordinates(point.lat(), point.lon())?));
    }
    let ascii_angle = query.contains(['\'', '"'])
        && upper
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace() || ".,+-;NSEW'\"".contains(c));
    if query.contains(['°', '′', '″']) || ascii_angle || looks_like_hemisphere_pair(&upper) {
        let (latitude, longitude) = if let Some(pair) = query.split_once([',', ';']) {
            pair
        } else {
            let boundary = upper
                .char_indices()
                .find(|(_, c)| matches!(c, 'N' | 'S'))
                .map(|(i, c)| i + c.len_utf8())
                .ok_or(CoordinateInputError::Format(ANGLE_INPUT_HELP))?;
            upper.split_at(boundary)
        };
        return Ok(Some(validate_coordinates(
            parse_angle(latitude, true)?,
            parse_angle(longitude, false)?,
        )?));
    }
    // A plain numeric pair is a coordinate attempt, including out-of-range/non-finite values.
    let cleaned = query.replace([',', ';'], " ");
    let numeric: Vec<_> = cleaned.split_whitespace().collect();
    if numeric.len() >= 2 && numeric.iter().all(|p| p.trim().parse::<f64>().is_ok()) {
        if numeric.len() != 2 {
            return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
        }
        return Ok(Some(validate_coordinates(
            numeric[0]
                .trim()
                .parse()
                .map_err(|_| CoordinateInputError::Format(ANGLE_INPUT_HELP))?,
            numeric[1]
                .trim()
                .parse()
                .map_err(|_| CoordinateInputError::Format(ANGLE_INPUT_HELP))?,
        )?));
    }
    Ok(None)
}
fn looks_like_hemisphere_pair(query: &str) -> bool {
    query
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_whitespace() || ".,+-;NSEW".contains(c))
        && query.contains(['N', 'S'])
        && query.contains(['E', 'W'])
}
fn parse_angle(text: &str, latitude: bool) -> Result<f64, CoordinateInputError> {
    let text = text.trim().to_uppercase();
    let hemisphere = text.chars().last().filter(|c| "NSEW".contains(*c));
    if let Some(hemisphere) = hemisphere
        && latitude != matches!(hemisphere, 'N' | 'S')
    {
        return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
    }
    let text = text.trim_end_matches(['N', 'S', 'E', 'W']).trim();
    let normalized = text.replace(['°', '′', '″', '\'', '"'], " ");
    let parts: Vec<f64> = normalized
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| CoordinateInputError::Format(ANGLE_INPUT_HELP))?;
    if parts.is_empty()
        || parts.len() > 3
        || parts.iter().any(|n| !n.is_finite())
        || parts.iter().skip(1).any(|&n| !(0.0..60.0).contains(&n))
    {
        return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
    }
    if parts.len() > 1 && parts[0].fract() != 0.0 {
        return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
    }
    if parts.len() == 3 && parts[1].fract() != 0.0 {
        return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
    }
    let negative = parts[0].is_sign_negative();
    if negative && matches!(hemisphere, Some('N' | 'E')) {
        return Err(CoordinateInputError::Format(ANGLE_INPUT_HELP));
    }
    let magnitude = parts[0].abs()
        + parts.get(1).copied().unwrap_or_default() / 60.0
        + parts.get(2).copied().unwrap_or_default() / 3600.0;
    Ok(if negative || matches!(hemisphere, Some('S' | 'W')) {
        -magnitude
    } else {
        magnitude
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angle_formats_parse_independently_of_display_and_keep_signs() {
        for input in [
            "37°46′29.64″N, 122°25′09.84″W",
            "37 46 29.64 N 122 25 9.84 W",
            "37°46.494′N; 122°25.164′W",
            "37°46'29.64\", -122°25'09.84\"",
        ] {
            let (lat, lon) = parse_coordinate_query(input).unwrap().unwrap();
            assert!((lat - 37.7749).abs() < 1e-10, "{input}");
            assert!((lon + 122.4194).abs() < 1e-10, "{input}");
        }
    }
    #[test]
    fn bad_coordinates_fail_locally_but_place_names_remain_searchable() {
        for input in [
            "91, 0",
            "0 181",
            "NaN, 0",
            "1,2,3",
            "37°60′N, 122°W",
            "37°59′60″N, 122°W",
            "-37°N, 122°E",
            "90°1′N, 0°E",
            "61 N 500000 5000000",
            "UPS Q 2000000 2000000",
            "18 N NaN 4000000",
            "18TWL1",
        ] {
            assert!(parse_coordinate_query(input).is_err(), "{input}");
        }
        for input in [
            "",
            "London",
            "Paris",
            "LAX",
            "San",
            "1600 Amphitheatre Parkway",
            "12 Main St",
            "King's Cross",
            "12 O'Connor St",
            "São Paulo",
            "東京",
        ] {
            assert!(parse_coordinate_query(input).unwrap().is_none(), "{input}");
        }
    }
}
