use super::*;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn screenshot_submission_retains_pixels_without_copying() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(PENDING_FILE_JOBS);
    let jobs = FileJobs { sender };
    let pixels = Arc::new(vec![10, 20, 30, 255]);
    jobs.screenshot(PathBuf::from("shot.png"), 1, 1, &pixels)
        .unwrap();
    let Job::Screenshot(_, _, _, retained) = receiver.try_recv().unwrap() else {
        panic!("screenshot job");
    };
    assert!(Arc::ptr_eq(&pixels, &retained));
}
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn jobs_preserve_order_snapshot_inputs_and_wake_after_success_and_failure() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "earthcraft-file-jobs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir_all(&fixture.0).unwrap();
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let (jobs, results) = FileJobs::new(runtime.handle(), move || {
        let _ = wake_tx.send(());
    });
    let kml = fixture.0.join("saved/marks.kml");
    let mut marks = vec![Placemark {
        name: "Saved snapshot".into(),
        desc: "".into(),
        lat: 20.0,
        lon: 30.0,
    }];
    jobs.save_kml(kml.clone(), &marks).unwrap();
    marks[0].name = "Edited later".into();
    jobs.open_kml(kml.clone(), Lang::En).unwrap();
    jobs.open_kml(fixture.0.join("missing.kml"), Lang::En)
        .unwrap();
    assert!(matches!(
        results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result,
        Ok(FileOutcome::KmlSaved { .. })
    ));
    let FileOutcome::KmlLoaded { path, placemarks } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("ordered KML load");
    };
    assert_eq!(path, kml);
    assert_eq!(placemarks[0].name, "Saved snapshot");
    assert!(results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .is_err());
    for _ in 0..3 {
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    let pixels = Arc::new(vec![10, 20, 30, 255]);
    jobs.screenshot(fixture.0.join("shot.png"), 1, 1, &pixels)
        .unwrap();
    drop(pixels);
    assert!(matches!(
        results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result,
        Ok(FileOutcome::ScreenshotSaved { .. })
    ));
    wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    let image = image::open(fixture.0.join("shot.png")).unwrap().to_rgba8();
    assert_eq!(image.as_raw(), &[10, 20, 30, 255]);
}
