//! Presentation-only formats; source coordinates remain double-precision degrees.
use super::{CoordinateInputError, validate_coordinates};
use mgrs_convert::{LatLon, Precision};
const MAX_ANGULAR_PRECISION: u8 = 8;
const MAX_GRID_PRECISION: u8 = 5;
const MINUTES_PER_DEGREE: f64 = 60.0;
const SECONDS_PER_DEGREE: f64 = 3600.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoordinateFormat {
    DecimalDegrees,
    #[default]
    Dms,
    Ddm,
    UtmUps,
    Mgrs,
}
impl CoordinateFormat {
    pub const ALL: [Self; 5] = [
        Self::DecimalDegrees,
        Self::Dms,
        Self::Ddm,
        Self::UtmUps,
        Self::Mgrs,
    ];
    pub fn code(self) -> &'static str {
        match self {
            Self::DecimalDegrees => "dd",
            Self::Dms => "dms",
            Self::Ddm => "ddm",
            Self::UtmUps => "utm_ups",
            Self::Mgrs => "mgrs",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.code() == code)
    }
    pub fn name_id(self) -> &'static str {
        match self {
            Self::DecimalDegrees => "coordinates-format-dd",
            Self::Dms => "coordinates-format-dms",
            Self::Ddm => "coordinates-format-ddm",
            Self::UtmUps => "coordinates-format-utm",
            Self::Mgrs => "coordinates-format-mgrs",
        }
    }
    pub fn precision_id(self) -> &'static str {
        match self {
            Self::DecimalDegrees => "coordinates-precision-degrees",
            Self::Dms => "coordinates-precision-seconds",
            Self::Ddm => "coordinates-precision-minutes",
            Self::UtmUps => "coordinates-precision-meters",
            Self::Mgrs => "coordinates-precision-grid",
        }
    }
    pub fn default_precision(self) -> u8 {
        match self {
            Self::DecimalDegrees | Self::Mgrs => 5,
            Self::Dms => 2,
            Self::Ddm => 3,
            Self::UtmUps => 1,
        }
    }
    pub fn precision_range(self) -> std::ops::RangeInclusive<u8> {
        match self {
            Self::Mgrs => 1..=MAX_GRID_PRECISION,
            _ => 0..=MAX_ANGULAR_PRECISION,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinateDisplay {
    pub format: CoordinateFormat,
    pub precision: u8,
}
impl Default for CoordinateDisplay {
    fn default() -> Self {
        Self {
            format: CoordinateFormat::Dms,
            precision: CoordinateFormat::Dms.default_precision(),
        }
    }
}
impl CoordinateDisplay {
    pub fn select_format(&mut self, format: CoordinateFormat) {
        if self.format != format {
            self.format = format;
            self.precision = format.default_precision();
        }
    }
    pub fn format(self, latitude: f64, longitude: f64) -> Result<String, CoordinateInputError> {
        let (latitude, longitude) = validate_coordinates(latitude, longitude)?;
        let range = self.format.precision_range();
        let precision = self.precision.clamp(*range.start(), *range.end()) as usize;
        Ok(match self.format {
            CoordinateFormat::DecimalDegrees => {
                format!("{latitude:.precision$}°, {longitude:.precision$}°")
            }
            CoordinateFormat::Dms | CoordinateFormat::Ddm => format!(
                "{}, {}",
                angle(latitude, true, self.format, precision),
                angle(longitude, false, self.format, precision)
            ),
            CoordinateFormat::UtmUps => {
                let point = LatLon::new(latitude, longitude)?.to_utmups();
                let hemisphere = if point.is_north() { "N" } else { "S" };
                let zone = if point.zone() == 0 {
                    "UPS".into()
                } else {
                    point.zone().to_string()
                };
                format!(
                    "{zone} {hemisphere} {:.precision$} {:.precision$}",
                    point.easting(),
                    point.northing()
                )
            }
            CoordinateFormat::Mgrs => LatLon::new(latitude, longitude)?
                .to_mgrs(Precision::new(precision as u8)?)
                .to_string(),
        })
    }
}
fn angle(value: f64, latitude: bool, format: CoordinateFormat, precision: usize) -> String {
    let hemisphere = match (latitude, value.is_sign_negative()) {
        (true, false) => 'N',
        (true, true) => 'S',
        (false, false) => 'E',
        (false, true) => 'W',
    };
    let scale = 10_f64.powi(precision as i32);
    let units = if format == CoordinateFormat::Dms {
        SECONDS_PER_DEGREE
    } else {
        MINUTES_PER_DEGREE
    };
    // Round the total before splitting so 59.999... carries to the next minute/degree.
    let rounded = (value.abs() * units * scale).round() / scale;
    let degrees = (rounded / units).floor() as u32;
    let remainder = (rounded - f64::from(degrees) * units).max(0.0);
    if format == CoordinateFormat::Dms {
        let minutes = (remainder / MINUTES_PER_DEGREE).floor() as u32;
        let seconds = remainder - f64::from(minutes) * MINUTES_PER_DEGREE;
        let width = if precision == 0 { 2 } else { precision + 3 };
        format!("{degrees}°{minutes:02}′{seconds:0width$.precision$}″{hemisphere}")
    } else {
        let width = if precision == 0 { 2 } else { precision + 3 };
        format!("{degrees}°{remainder:0width$.precision$}′{hemisphere}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinates::parse_coordinate_query;
    #[test]
    fn display_carries_seconds_and_minutes_without_mutating_values() {
        let position = (12.999_999_999, -179.999_999_999);
        assert_eq!(
            CoordinateDisplay::default()
                .format(position.0, position.1)
                .unwrap(),
            "13°00′00.00″N, 180°00′00.00″W"
        );
        let display = CoordinateDisplay {
            format: CoordinateFormat::Ddm,
            precision: 3,
        };
        assert_eq!(
            display.format(position.0, position.1).unwrap(),
            "13°00.000′N, 180°00.000′W"
        );
        assert_eq!(position, (12.999_999_999, -179.999_999_999));
        assert_eq!(
            CoordinateDisplay::default().format(-0.0, -0.0).unwrap(),
            "0°00′00.00″N, 0°00′00.00″E"
        );
    }
    #[test]
    fn grid_formats_match_the_upstream_empire_state_building_reference() {
        // geoconvert 1.0.2 LatLon/UtmUps documentation, independent fixed reference.
        let position = (40.748333, -73.985278);
        let display = CoordinateDisplay {
            format: CoordinateFormat::UtmUps,
            precision: 3,
        };
        assert_eq!(
            display.format(position.0, position.1).unwrap(),
            "18 N 585664.121 4511315.422"
        );
        let display = CoordinateDisplay {
            format: CoordinateFormat::Mgrs,
            precision: 5,
        };
        assert_eq!(
            display.format(position.0, position.1).unwrap(),
            "18TWL8566411315"
        );
    }
    #[test]
    fn all_formats_roundtrip_and_handle_poles_dateline_and_utm_exceptions() {
        for format in CoordinateFormat::ALL {
            let display = CoordinateDisplay {
                format,
                precision: format.default_precision(),
            };
            for (latitude, longitude) in [
                (37.7749, -122.4194),
                (-33.8688, 151.2093),
                (89.0, 45.0),
                (-85.0, -30.0),
                (60.0, 6.0),
                (78.0, 20.0),
            ] {
                let text = display.format(latitude, longitude).unwrap();
                let (lat, lon) = parse_coordinate_query(&text)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{text}"));
                assert!(
                    (lat - latitude).abs() < 0.0001,
                    "{text}: {lat} != {latitude}"
                );
                assert!(
                    (lon - longitude).abs() < 0.001,
                    "{text}: {lon} != {longitude}"
                );
            }
            for position in [(90.0, 0.0), (-90.0, 180.0), (0.0, 180.0), (0.0, -180.0)] {
                assert!(!display.format(position.0, position.1).unwrap().is_empty());
            }
        }
        let display = CoordinateDisplay {
            format: CoordinateFormat::UtmUps,
            precision: 0,
        };
        assert!(display.format(60.0, 6.0).unwrap().starts_with("32 N"));
        assert!(display.format(78.0, 20.0).unwrap().starts_with("33 N"));
        assert!(display.format(-80.001, 0.0).unwrap().starts_with("UPS S"));
        assert!(display.format(84.0, 0.0).unwrap().starts_with("UPS N"));
        assert!(display.format(91.0, 0.0).is_err());
    }
}
