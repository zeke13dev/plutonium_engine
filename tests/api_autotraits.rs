// api_autotraits.rs — static auto-trait guard for the plutonium_engine public surface.
//
// PURPOSE: This file will FAIL TO COMPILE if a future refactor accidentally removes Send+Sync
// from a public type that currently implements both traits. The assertions are compile-time only;
// there is nothing to run at test time.
//
// NEGATIVE BOUNDS (PlutoniumEngine, PlutoniumApp, …):
// Rust does not support a direct compile-time assertion that a type is *not* Send/Sync.
// These types are !Send/!Sync because they hold `Rc<RefCell<…>>` and non-Send wgpu handles.
// That invariant is documented here; if a future refactor tries to make them Send by replacing
// Rc with Arc, the reviewer must consciously remove the doc comment and re-evaluate the policy.
//
// POSITIVE BOUNDS:
// Plain-data (POD) public types that are currently Send+Sync are asserted below.  If any of
// these types gains a non-Send field (e.g. a raw pointer or Rc), the assertion function below
// will fail at compile time with a "T: Send" unsatisfied bound error.

use plutonium_engine::{
    DrawParams, FontLoadOptions, GlyphSet, HaloFalloff, HaloMode, HaloPreset, HaloStyle,
    PrewarmConfig, PrewarmPolicy, RasterHintingMode, TextureFit, WarmStats,
};

use plutonium_engine::app::{FrameContext, WindowConfig};

use plutonium_engine::popup::{
    PopupAction, PopupActionStyle, PopupConfig, PopupDismissReason, PopupEvent, PopupSize,
};

/// Compile-time assertion: T must implement both Send and Sync.
///
/// If a type listed below loses Send or Sync, this function will fail to compile for that type.
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_pod_types_are_send_and_sync() {
    // Primitive params / config bags — all Copy or Clone POD, must remain Send+Sync.
    assert_send_sync::<DrawParams>();
    assert_send_sync::<TextureFit>();
    assert_send_sync::<WarmStats>();
    assert_send_sync::<FontLoadOptions>();
    assert_send_sync::<RasterHintingMode>();
    assert_send_sync::<GlyphSet>();
    assert_send_sync::<PrewarmPolicy>();
    assert_send_sync::<PrewarmConfig>();

    // Halo / highlight types — Copy structs / enums defined in lib.rs.
    assert_send_sync::<HaloFalloff>();
    assert_send_sync::<HaloMode>();
    assert_send_sync::<HaloPreset>();
    assert_send_sync::<HaloStyle>();

    // App config — simple fields (String, u32); must remain thread-safe.
    assert_send_sync::<WindowConfig>();

    // Frame context — only POD fields; must remain Send+Sync.
    assert_send_sync::<FrameContext>();

    // Popup config and related plain types.
    assert_send_sync::<PopupConfig>();
    assert_send_sync::<PopupSize>();
    assert_send_sync::<PopupAction>();
    assert_send_sync::<PopupActionStyle>();
    assert_send_sync::<PopupDismissReason>();
    assert_send_sync::<PopupEvent>();
}

// --------------------------------------------------------------------------
// INTENTIONALLY !Send / !Sync types — documented here, NOT asserted.
//
// The following public types are INTENTIONALLY not Send or Sync because they
// contain `Rc<RefCell<…>>` internals (pluto_objects) or non-Send wgpu resources
// (PlutoniumEngine, PlutoniumApp). Do NOT add them to `assert_send_sync` above:
//
//   plutonium_engine::PlutoniumEngine<'_>       — holds Rc + wgpu::Device (non-Send)
//   plutonium_engine::app::PlutoniumApp         — owns PlutoniumEngine + Rc callback
//   plutonium_engine::pluto_objects::*::Shape   — wraps Rc<RefCell<ShapeInternal>>
//   plutonium_engine::pluto_objects::*::Text2D  — wraps Rc<RefCell<…>>
//   plutonium_engine::pluto_objects::*::Texture2D / TextureAtlas2D
//   plutonium_engine::pluto_objects::button::Button / ButtonInternal
//   plutonium_engine::pluto_objects::text_input::TextInput / TextInputInternal
//   plutonium_engine::anim::Timeline<T>         — contains Rc<…> callbacks
//   plutonium_engine::text::FontAtlas / TextRenderer / TinyRasterFallbackSpec
//
// If a future refactor converts these from Rc to Arc to enable Send, that change
// should be an explicit design decision, not an accidental side-effect.
// --------------------------------------------------------------------------
