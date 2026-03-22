use gateway::dedup::fingerprint::Deduplicator;

#[test]
fn first_request_passes() {
    let dedup = Deduplicator::new(60);
    assert!(dedup.check("hash-abc", "req-1").is_ok());
}

#[test]
fn duplicate_within_window_rejected() {
    let dedup = Deduplicator::new(60);
    dedup.check("hash-abc", "req-1").unwrap();
    let err = dedup.check("hash-abc", "req-2").unwrap_err();
    assert_eq!(err, "req-1");
}

#[test]
fn different_hash_allowed() {
    let dedup = Deduplicator::new(60);
    dedup.check("hash-abc", "req-1").unwrap();
    assert!(dedup.check("hash-xyz", "req-2").is_ok());
}

#[test]
fn expired_entry_allows_resubmission() {
    // Use a 0-second window so entries expire immediately
    let dedup = Deduplicator::new(0);
    dedup.check("hash-abc", "req-1").unwrap();
    // With 0-second TTL, the entry is already expired on next check
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(dedup.check("hash-abc", "req-2").is_ok());
}

#[test]
fn multiple_distinct_hashes_coexist() {
    let dedup = Deduplicator::new(60);
    for i in 0..10 {
        let hash = format!("hash-{i}");
        let req_id = format!("req-{i}");
        assert!(dedup.check(&hash, &req_id).is_ok());
    }
    // All should be rejected on second attempt
    for i in 0..10 {
        let hash = format!("hash-{i}");
        assert!(dedup.check(&hash, "req-dup").is_err());
    }
}
