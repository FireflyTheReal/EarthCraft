//! Illustrative flight routes and time-based geographic positions.
use crate::math::wrap_lon;
#[derive(Clone, Copy)]
struct FlightRoute {
    code: &'static str,
    from: (f64, f64),
    to: (f64, f64),
    period_secs: f64,
}

const FLIGHT_ROUTES: &[FlightRoute] = &[
    FlightRoute {
        code: "AZU2410",
        from: (-23.55, -46.64),
        to: (-22.91, -43.17),
        period_secs: 2400.0,
    },
    FlightRoute {
        code: "GLO2056",
        from: (-22.81, -43.25),
        to: (-12.97, -38.50),
        period_secs: 4300.0,
    },
    FlightRoute {
        code: "LAT3401",
        from: (-23.43, -46.47),
        to: (-30.03, -51.23),
        period_secs: 5000.0,
    },
    FlightRoute {
        code: "AFR459",
        from: (48.86, 2.35),
        to: (-23.55, -46.64),
        period_secs: 9000.0,
    },
    FlightRoute {
        code: "BAW247",
        from: (51.47, -0.45),
        to: (-22.91, -43.17),
        period_secs: 8600.0,
    },
    FlightRoute {
        code: "DAL105",
        from: (33.64, -84.42),
        to: (-23.43, -46.47),
        period_secs: 9500.0,
    },
    FlightRoute {
        code: "UAL905",
        from: (40.64, -73.78),
        to: (51.47, -0.45),
        period_secs: 4200.0,
    },
    FlightRoute {
        code: "AAL170",
        from: (25.79, -80.29),
        to: (40.64, -73.78),
        period_secs: 3800.0,
    },
    FlightRoute {
        code: "QFA12",
        from: (-33.94, 151.18),
        to: (34.05, -118.24),
        period_secs: 10500.0,
    },
    FlightRoute {
        code: "JAL701",
        from: (35.55, 139.78),
        to: (1.36, 103.99),
        period_secs: 5100.0,
    },
    FlightRoute {
        code: "UAE261",
        from: (25.25, 55.36),
        to: (51.47, -0.45),
        period_secs: 4700.0,
    },
    FlightRoute {
        code: "ETH506",
        from: (8.98, 38.79),
        to: (-26.13, 28.23),
        period_secs: 4200.0,
    },
    FlightRoute {
        code: "SAA223",
        from: (-26.13, 28.23),
        to: (-33.94, 18.60),
        period_secs: 2600.0,
    },
    FlightRoute {
        code: "ACA901",
        from: (43.68, -79.63),
        to: (49.19, -123.18),
        period_secs: 5300.0,
    },
    FlightRoute {
        code: "ANZ32",
        from: (-36.84, 174.76),
        to: (-33.94, 151.18),
        period_secs: 2400.0,
    },
    FlightRoute {
        code: "TAP074",
        from: (38.77, -9.13),
        to: (-23.55, -46.64),
        period_secs: 8800.0,
    },
];

fn route_position(route: FlightRoute, t: f64) -> (f64, f64) {
    let lat = route.from.0 + (route.to.0 - route.from.0) * ease_in_out(t);
    let delta_lon = wrap_lon(route.to.1 - route.from.1);
    let lon = wrap_lon(route.from.1 + delta_lon * ease_in_out(t));
    (lat, lon)
}

fn ease_in_out(t: f64) -> f64 {
    0.5 - (std::f64::consts::PI * t).cos() * 0.5
}

/// One sample of the built-in illustrative routes, not live aircraft telemetry.
pub struct FlightSample {
    pub code: &'static str,
    pub position: (f64, f64),
    pub next_position: (f64, f64),
    pub flight_level: i32,
}
pub fn sample_flights(unix_secs: f64) -> impl Iterator<Item = FlightSample> {
    FLIGHT_ROUTES.iter().map(move |route| {
        let progress = (unix_secs / route.period_secs).fract();
        FlightSample {
            code: route.code,
            position: route_position(*route, progress),
            next_position: route_position(*route, (progress + HEADING_LOOKAHEAD).fract()),
            flight_level: BASE_FLIGHT_LEVEL + ((progress * FLIGHT_LEVEL_RANGE) as i32),
        }
    })
}
const HEADING_LOOKAHEAD: f64 = 0.002;
const BASE_FLIGHT_LEVEL: i32 = 300;
const FLIGHT_LEVEL_RANGE: f64 = 80.0;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_preserve_endpoints_midpoint_and_period() {
        let first = sample_flights(0.0).next().unwrap();
        assert_eq!(first.code, "AZU2410");
        assert!((first.position.0 + 23.55).abs() < 1e-12);
        assert!((first.position.1 + 46.64).abs() < 1e-12);
        assert_eq!(first.flight_level, 300);
        let middle = sample_flights(1200.0).next().unwrap();
        assert!((middle.position.0 + 23.23).abs() < 1e-12);
        assert!((middle.position.1 + 44.905).abs() < 1e-12);
        assert_eq!(
            sample_flights(2400.0).next().unwrap().position,
            first.position
        );
    }
    #[test]
    fn routes_take_the_short_path_across_the_dateline() {
        let route = FlightRoute {
            code: "test",
            from: (0.0, 170.0),
            to: (0.0, -170.0),
            period_secs: 1.0,
        };
        assert_eq!(route_position(route, 0.5).1.abs(), 180.0);
    }
}
