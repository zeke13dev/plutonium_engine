use plutonium_engine::rng::RngService;

#[test]
fn deterministic_streams() {
    let svc = RngService::with_seed(12345);
    let mut a1 = svc.derive_stream(7);
    let mut a2 = svc.derive_stream(7);
    let mut b = svc.derive_stream(8);
    // Same stream id -> same sequence
    for _ in 0..10 {
        assert_eq!(a1.next_u64(), a2.next_u64());
    }
    // Different stream id -> likely differ
    let x = a1.next_u64();
    let y = b.next_u64();
    assert_ne!(x, y);
}

#[test]
fn shuffle_is_stable_with_seed() {
    let svc = RngService::with_seed(42);
    let mut s1 = svc.derive_stream(1);
    let mut s2 = svc.derive_stream(1);
    let mut a = (0..16).collect::<Vec<_>>();
    let mut b = (0..16).collect::<Vec<_>>();
    s1.shuffle(&mut a);
    s2.shuffle(&mut b);
    assert_eq!(a, b);
}
