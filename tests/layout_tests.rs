#![cfg(feature = "layout")]
use plutonium_engine::layout::{layout_node, Anchors, HAnchor, LayoutParams, PercentSize, VAnchor};
use plutonium_engine::utils::{Rectangle, Size};

#[test]
fn layout_with_margins_and_percent() {
    let container = Rectangle::new(0.0, 0.0, 400.0, 200.0);
    let desired = Size {
        width: 100.0,
        height: 50.0,
    };
    let params = LayoutParams {
        anchors: Anchors {
            h: HAnchor::Right,
            v: VAnchor::Bottom,
        },
        percent: Some(PercentSize {
            width_pct: 0.25,
            height_pct: 0.5,
        }),
        margins: plutonium_engine::layout::Margins {
            left: 0.0,
            right: 10.0,
            top: 0.0,
            bottom: 10.0,
        },
    };
    let out = layout_node(container, desired, params);
    assert!((out.size.width - 100.0).abs() < 1e-3);
    assert!((out.size.height - 100.0).abs() < 1e-3);
    // Bottom-right anchored with 10px margins
    assert!((out.position.x - (400.0 - 10.0 - out.size.width)).abs() < 1e-3);
    assert!((out.position.y - (200.0 - 10.0 - out.size.height)).abs() < 1e-3);
}
