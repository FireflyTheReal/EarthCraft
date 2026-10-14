use super::*;
use snapshot::{Samples, prepare};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

fn flight(hex: &str, observed: f64, latitude: f64) -> Aircraft {
    Aircraft {
        hex: hex.into(),
        callsign: hex.into(),
        latitude,
        longitude: 0.0,
        heading_deg: 90.0,
        speed_knots: Some(400.0),
        altitude_feet: None,
        observed_unix: observed,
    }
}
fn samples(aircraft: Vec<Aircraft>, now: Instant) -> Samples {
    Samples {
        aircraft: aircraft.into(),
        status: ProviderStatus {
            loaded: Some(now),
            ..Default::default()
        },
    }
}

#[test]
fn merge_retains_source_priority_sort_order_and_expiry_falls_back_to_global() {
    let now = Instant::now();
    let global = samples(
        vec![flight("bb", 980.0, 2.0), flight("aa", 900.0, 1.0)],
        now,
    );
    let local = samples(
        vec![
            flight("aa", 950.0, 3.0),
            flight("bb", 970.0, 4.0),
            flight("cc", 800.0, 5.0),
        ],
        now,
    );
    let merged = prepare(global.clone(), local.clone(), now, 1000.0);
    assert_eq!(
        merged
            .snapshot
            .aircraft
            .iter()
            .map(|f| f.hex.as_ref())
            .collect::<Vec<_>>(),
        ["aa", "bb"]
    );
    assert_eq!(merged.snapshot.aircraft[0].latitude, 3.0);
    assert_eq!(merged.snapshot.aircraft[1].latitude, 2.0);
    let expired = prepare(
        global.clone(),
        local.clone(),
        now + Duration::from_secs(71),
        1071.0,
    );
    assert_eq!(expired.snapshot.aircraft[0].latitude, 1.0);
    let gone = prepare(global, local, now + Duration::from_secs(1201), 2201.0);
    assert!(gone.snapshot.aircraft.is_empty());
}

#[test]
fn snapshot_reads_share_the_same_data_and_a_stopped_service_reports_an_error() {
    let (control, _) = watch::channel(AircraftQuery::default());
    let latest = Arc::new(AircraftSnapshot {
        aircraft: vec![flight("aa", 1000.0, 0.0)].into(),
        ..Default::default()
    });
    let (publish, updates) = watch::channel(latest.clone());
    let mut service = LiveAircraft {
        control,
        updates,
        latest,
        stopped: false,
    };
    let first = service.snapshot();
    assert!(Arc::ptr_eq(&first, &service.snapshot()));
    drop(publish);
    service.update(AircraftQuery::default());
    assert!(service.snapshot().error.is_some());
    assert!(Arc::ptr_eq(&first.aircraft, &service.snapshot().aircraft));
}

fn server(status: &str, body: String) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_owned();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        String::from_utf8(request).unwrap()
    });
    (format!("http://{address}"), thread)
}

#[test]
fn live_service_fetches_both_sources_off_ui_and_publishes_without_duplicate_requests() {
    let now = earthcraft_core::math::now_unix();
    let (global_url, global) = server(
        "200 OK",
        serde_json::json!({"states":[
            ["aa", "GLOBAL", "x", now, now, 1.0, 2.0, 1000, false, 200.0, 90.0],
            ["bb", "OTHER", "x", now, now, 2.0, 3.0, 1000, false, 200.0, 90.0]
        ]})
        .to_string(),
    );
    let (local_url, local) = server(
        "200 OK",
        serde_json::json!({"ac":[
            {"hex":"aa","flight":"LOCAL","lat":1.0,"lon":2.0,"seen_pos":0,"track":90.0}
        ]})
        .to_string(),
    );
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake, wakes) = mpsc::channel();
    let ui_thread = std::thread::current().id();
    let mut service = LiveAircraft::with_endpoints(
        runtime.handle(),
        move || {
            assert_ne!(ui_thread, std::thread::current().id());
            let _ = wake.send(());
        },
        worker::Endpoints {
            global: format!("{global_url}/states"),
            local: format!("{local_url}/v2"),
        },
    );
    let query = AircraftQuery {
        enabled: true,
        latitude: 10.0,
        longitude: 20.0,
        altitude_km: 1000.0,
    };
    loop {
        service.update(query);
        let snapshot = service.snapshot();
        if snapshot.global.loaded.is_some() && snapshot.local.loaded.is_some() {
            assert_eq!(snapshot.aircraft.len(), 2);
            assert_eq!(snapshot.aircraft[0].callsign.as_ref(), "LOCAL");
            break;
        }
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    for _ in 0..100 {
        service.update(query);
    }
    assert!(global.join().unwrap().starts_with("GET /states "));
    assert!(
        local
            .join()
            .unwrap()
            .starts_with("GET /v2/lat/10.0000/lon/20.0000/dist/250 ")
    );
}

#[test]
fn service_wakes_after_http_failure_and_does_not_fetch_local_above_its_altitude_limit() {
    let (url, server) = server("503 Service Unavailable", String::new());
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake, wakes) = mpsc::channel();
    let mut service = LiveAircraft::with_endpoints(
        runtime.handle(),
        move || {
            let _ = wake.send(());
        },
        worker::Endpoints {
            global: url,
            local: "http://127.0.0.1:1".into(),
        },
    );
    let query = AircraftQuery {
        enabled: true,
        altitude_km: 14_000.0,
        ..Default::default()
    };
    loop {
        service.update(query);
        let snapshot = service.snapshot();
        if let Some(error) = &snapshot.global.error {
            assert!(matches!(error.as_ref(), crate::Error::Http(_)));
            assert!(!snapshot.local.attempted);
            break;
        }
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    server.join().unwrap();
}

#[test]
fn disabling_pauses_expiry_publication_and_enabling_discards_expired_positions() {
    let (global_url, global) = server("200 OK", "{\"states\":[]}".into());
    let (local_url, local) = server(
        "200 OK",
        serde_json::json!({"ac":[
            {"hex":"aa","lat":1.0,"lon":2.0,"seen_pos":119.5}
        ]})
        .to_string(),
    );
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake, wakes) = mpsc::channel();
    let mut service = LiveAircraft::with_endpoints(
        runtime.handle(),
        move || {
            let _ = wake.send(());
        },
        worker::Endpoints {
            global: global_url,
            local: local_url,
        },
    );
    let mut query = AircraftQuery {
        enabled: true,
        altitude_km: 1000.0,
        ..Default::default()
    };
    loop {
        service.update(query);
        let snapshot = service.snapshot();
        if snapshot.global.loaded.is_some() && snapshot.local.loaded.is_some() {
            break;
        }
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    assert_eq!(service.snapshot().aircraft.len(), 1);
    query.enabled = false;
    service.update(query);
    let disabled = service.snapshot();
    std::thread::sleep(Duration::from_millis(1250));
    service.update(query);
    assert!(Arc::ptr_eq(&disabled, &service.snapshot()));
    query.enabled = true;
    service.update(query);
    let waiting = service.snapshot();
    assert!(
        waiting.aircraft.is_empty(),
        "expired positions must not flash back on resume"
    );
    loop {
        service.update(query);
        if !Arc::ptr_eq(&waiting, &service.snapshot()) {
            assert!(service.snapshot().aircraft.is_empty());
            break;
        }
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    global.join().unwrap();
    local.join().unwrap();
}
