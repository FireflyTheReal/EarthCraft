use super::*;
use crate::runtime::ServiceRuntime;
fn service(runtime: &ServiceRuntime) -> UpdateService {
    UpdateService::configured(runtime.handle(), Arc::new(|| {}), true, None)
}
#[test]
fn disabled_startup_and_duplicate_actions_do_not_launch_jobs() {
    let runtime = ServiceRuntime::new().unwrap();
    let mut service = service(&runtime);
    service.start_once();
    assert_eq!(service.state(), &UpdateState::Idle);
    for state in [
        UpdateState::Checking,
        UpdateState::UpToDate,
        UpdateState::Downloading {
            version: "1.0.0".into(),
            downloaded: 12,
            total: None,
        },
        UpdateState::Installing {
            version: "1.0.0".into(),
        },
        UpdateState::ReadyToRestart {
            version: "1.0.0".into(),
        },
    ] {
        service.state = state.clone();
        service.activate();
        service.start_once();
        assert_eq!(service.state(), &state);
    }
}
#[test]
fn progress_is_coalesced_and_final_completion_is_visible() {
    let runtime = ServiceRuntime::new().unwrap();
    let mut service = service(&runtime);
    for downloaded in 0..1000 {
        service.sender.send_replace(Snapshot {
            state: UpdateState::Downloading {
                version: "1.0.0".into(),
                downloaded,
                total: Some(1000),
            },
            selected: None,
        });
    }
    service.poll();
    assert!(matches!(
        service.state(),
        UpdateState::Downloading {
            downloaded: 999,
            ..
        }
    ));
    service.sender.send_replace(Snapshot {
        state: UpdateState::ReadyToRestart {
            version: "1.0.0".into(),
        },
        selected: None,
    });
    service.poll();
    assert!(matches!(
        service.state(),
        UpdateState::ReadyToRestart { .. }
    ));
    service.activate();
    service.start_once();
    assert!(matches!(
        service.state(),
        UpdateState::ReadyToRestart { .. }
    ));
}
