use gateway::kill_switch::controller::KillSwitchController;
use gateway::types::KillSwitchState;

#[test]
fn starts_operational() {
    let ks = KillSwitchController::new(5, true);
    assert_eq!(ks.state(), KillSwitchState::Operational);
    assert!(ks.is_operational());
}

#[test]
fn trigger_drain() {
    let ks = KillSwitchController::new(5, true);
    ks.trigger_drain();
    assert_eq!(ks.state(), KillSwitchState::Drain);
    assert!(!ks.is_operational());
}

#[test]
fn trigger_halt() {
    let ks = KillSwitchController::new(5, true);
    ks.trigger_halt();
    assert_eq!(ks.state(), KillSwitchState::Halted);
    assert!(!ks.is_operational());
}

#[test]
fn drain_then_halt() {
    let ks = KillSwitchController::new(5, true);
    ks.trigger_drain();
    assert_eq!(ks.state(), KillSwitchState::Drain);
    ks.trigger_halt();
    assert_eq!(ks.state(), KillSwitchState::Halted);
}

#[test]
fn resume_from_drain() {
    let ks = KillSwitchController::new(5, true);
    ks.trigger_drain();
    let resumed = ks.resume();
    assert!(resumed);
    assert_eq!(ks.state(), KillSwitchState::Operational);
}

#[test]
fn resume_from_halt() {
    let ks = KillSwitchController::new(5, true);
    ks.trigger_halt();
    let resumed = ks.resume();
    assert!(resumed);
    assert_eq!(ks.state(), KillSwitchState::Operational);
}

#[test]
fn resume_when_already_operational_returns_false() {
    let ks = KillSwitchController::new(5, true);
    let resumed = ks.resume();
    assert!(!resumed);
}

#[test]
fn auto_drain_on_consecutive_criticals() {
    let threshold = 3;
    let ks = KillSwitchController::new(threshold, false);

    // First two don't trigger
    assert!(!ks.notify_critical());
    assert!(!ks.notify_critical());
    assert!(ks.is_operational());

    // Third triggers auto-drain
    assert!(ks.notify_critical());
    assert_eq!(ks.state(), KillSwitchState::Drain);
}

#[test]
fn reset_criticals_prevents_auto_drain() {
    let ks = KillSwitchController::new(3, false);
    ks.notify_critical();
    ks.notify_critical();
    ks.reset_criticals();
    // Counter is reset, so this is critical #1 again
    assert!(!ks.notify_critical());
    assert!(!ks.notify_critical());
    assert!(ks.is_operational());
    // Now third consecutive triggers
    assert!(ks.notify_critical());
    assert_eq!(ks.state(), KillSwitchState::Drain);
}

#[test]
fn credential_scrub_auto_halt_enabled() {
    let ks = KillSwitchController::new(5, true);
    let halted = ks.notify_credential_scrub();
    assert!(halted);
    assert_eq!(ks.state(), KillSwitchState::Halted);
}

#[test]
fn credential_scrub_auto_halt_disabled() {
    let ks = KillSwitchController::new(5, false);
    let halted = ks.notify_credential_scrub();
    assert!(!halted);
    assert_eq!(ks.state(), KillSwitchState::Operational);
}

#[test]
fn resume_resets_critical_counter() {
    let ks = KillSwitchController::new(3, false);
    ks.notify_critical();
    ks.notify_critical();
    ks.trigger_drain();
    ks.resume();
    // Counter should be reset, so need 3 more criticals
    assert!(!ks.notify_critical());
    assert!(!ks.notify_critical());
    assert!(ks.is_operational());
    assert!(ks.notify_critical());
    assert_eq!(ks.state(), KillSwitchState::Drain);
}
