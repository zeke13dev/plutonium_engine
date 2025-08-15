use plutonium_engine::utils::Position;
use plutonium_engine::{
    app::{run_app, FrameContext, WindowConfig},
    PlutoniumEngine,
};
use plutonium_game_assets::{
    load_manifest, process_load_requests, AssetsRegistry, LoadRequests, TextureLoadRequest,
};
use plutonium_game_audio::Audio;
use plutonium_game_core::{
    ease_value, App as GameApp, Ease, Entity, SceneSystems, Time, TweenAlpha, TweenPosition,
    TweenScale,
};
use plutonium_game_input::{ActionMap, InputState};
use plutonium_game_ui::{
    draw_panel_9slice_tiled, render_system, Button, DrawParams, FadeOverlay, FocusManager,
    RenderCommands, RenderOffset, Slider, StackCrossAlign, StackDirection, StackLayout,
    TextCommits, TextInput, Theme, Toggle,
};
use std::path::Path;

// RNG-driven deal grid resources/components
#[derive(Clone, Copy)]
struct CardTag;
#[derive(Clone, Copy)]
struct PositionComp {
    x: f32,
    y: f32,
}
#[derive(Clone, Copy)]
struct Velocity {
    x: f32,
    y: f32,
}
#[derive(Clone, Copy)]
struct Player(Entity);
#[derive(Clone, Copy)]
enum Scene {
    Menu,
    Game,
}
#[derive(Clone)]
struct Cards {
    ids: Vec<Entity>,
}
struct CardTexture(uuid::Uuid);
struct DealPending {
    entity: Entity,
    delay: f32,
    to: (f32, f32),
    duration: f32,
}
struct DealState {
    pending: Vec<DealPending>,
    started: bool,
    rng: plutonium_game_core::Rng64,
}

fn main() {
    let mut game = GameApp::new();
    game.world.insert_resource(Time::default());
    // resources
    let mut assets = AssetsRegistry::new();
    game.world.insert_resource(RenderCommands::default());
    // input resources
    game.world.insert_resource(InputState::default());
    let mut actions = ActionMap::default();
    actions.bind("toggle", "Space");
    actions.bind("start", "Enter");
    game.world.insert_resource(actions);
    game.world.insert_resource(LoadRequests::default());
    // spawn via startup system
    game.startup.add_system(|world| {
        let e: Entity = world.spawn();
        world.insert_component(e, PositionComp { x: 100.0, y: 100.0 });
        world.insert_component(e, Velocity { x: 30.0, y: 0.0 });
        world.insert_component(e, TweenScale::new(1.0, 1.5, 2.0));
        world.insert_component(e, TweenAlpha::new(0.0, 1.0, 1.0));
        world.insert_component(e, TweenPosition::new((100.0, 100.0), (200.0, 100.0), 3.0));
        world.insert_resource(Player(e));
        // Initialize scene stack to Menu
        plutonium_game_core::scene_push(world, "Menu");
    });
    game.run_startup();

    // fixed update system: apply velocity
    game.fixed_update.add_system(|world| {
        let dt = world
            .get_resource::<Time>()
            .map(|t| t.delta_seconds)
            .unwrap_or(0.0);
        if let Some(player) = world.get_resource::<Player>().copied() {
            let vel = world.get_component::<Velocity>(player.0).copied();
            if let Some(pos) = world.get_component_mut::<PositionComp>(player.0) {
                if let Some(vel) = vel {
                    pos.x += vel.x * dt;
                    pos.y += vel.y * dt;
                }
            }
        }
    });

    let mut is_red = false;
    let mut fixed_accum = 0.0f32;
    const FIXED_STEP: f32 = 1.0 / 60.0;
    let mut start_button = Button::new(20.0, 60.0, 200.0, 36.0, "Start Game", "roboto");
    let mut sound_toggle = Toggle::new(20.0, 100.0, 200.0, 36.0, false, "Sound", "roboto");
    let mut volume_slider = Slider::new(20.0, 140.0, 200.0, 36.0, 0.4, "Volume", "roboto");
    let mut name_input = TextInput::new(20.0, 180.0, 240.0, 36.0, "roboto");
    game.world.insert_resource(Theme::default());
    game.world.insert_resource(FocusManager::default());
    game.world.insert_resource(SceneSystems::default());
    game.world.insert_resource(Audio::new());
    // Metrics toggle (simple)
    #[derive(Clone, Copy)]
    struct Metrics {
        enabled: bool,
    }
    game.world.insert_resource(Metrics { enabled: false });
    // Register simple per-scene update schedules
    {
        let scenes = game.world.get_resource_mut::<SceneSystems>().unwrap();
        let menu_update = plutonium_game_core::Schedule::new().with_system(|world| {
            if let Some(cmds) = world.get_resource_mut::<RenderCommands>() {
                cmds.draw_text(
                    "roboto".into(),
                    "[Menu]".into(),
                    Position { x: 260.0, y: 20.0 },
                    (60.0, 20.0),
                );
            }
        });
        let game_update = plutonium_game_core::Schedule::new().with_system(|world| {
            if let Some(cmds) = world.get_resource_mut::<RenderCommands>() {
                cmds.draw_text(
                    "roboto".into(),
                    "[Game]".into(),
                    Position { x: 230.0, y: 20.0 },
                    (60.0, 20.0),
                );
            }
        });
        scenes.register_update("Menu", menu_update);
        scenes.register_update("Game", game_update);
    }
    // Resolve a workspace-relative path (repo root) from this example crate
    fn ws_path(rel: &str) -> String {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = base.ancestors().nth(3).unwrap_or(base);
        root.join(rel).to_string_lossy().to_string()
    }

    let _ = run_app(
        WindowConfig::default(),
        move |engine: &mut PlutoniumEngine, frame: &FrameContext| {
            // Startup-like: queue load request exactly once
            static mut TEX_HANDLE: Option<(plutonium_game_assets::Handle, uuid::Uuid)> = None;
            static mut BTN_BG_HANDLE: Option<(plutonium_game_assets::Handle, uuid::Uuid)> = None;
            if unsafe { TEX_HANDLE.is_none() } {
                // load manifest for theme/fonts
                if let Ok(manifest) =
                    load_manifest(&format!("{}/manifest.toml", env!("CARGO_MANIFEST_DIR")))
                {
                    // set theme
                    if let Some(theme) = game.world.get_resource_mut::<Theme>() {
                        theme.primary_text_rgba = manifest.theme.primary_text_rgba;
                        theme.button_bg_rgba = manifest.theme.button_bg_rgba;
                        theme.button_bg_hover_rgba = manifest.theme.button_bg_hover_rgba;
                    }
                    // preload fonts
                    for f in manifest.fonts {
                        let font_path = if Path::new(&f.path).is_file() {
                            f.path.clone()
                        } else {
                            ws_path(&f.path)
                        };
                        let _ = engine.load_font(&font_path, f.size, &f.key);
                    }
                    // queue textures
                    if !manifest.textures.is_empty() {
                        let loads = game.world.get_resource_mut::<LoadRequests>().unwrap();
                        for t in manifest.textures {
                            let handle = assets.reserve_handle();
                            assets.set_named_handle(&t.name, handle);
                            let tex_path = if Path::new(&t.path).is_file() {
                                t.path
                            } else {
                                ws_path(&t.path)
                            };
                            loads.textures.push(TextureLoadRequest {
                                handle,
                                file_path: tex_path,
                                position: Position { x: 0.0, y: 0.0 },
                                scale_factor: 1.0,
                            });
                        }
                    }
                    // queue panels as atlases
                    if !manifest.panels.is_empty() {
                        let loads = game.world.get_resource_mut::<LoadRequests>().unwrap();
                        for p in manifest.panels {
                            let handle = assets.reserve_handle();
                            assets.set_named_atlas_handle(&p.name, handle);
                            let pan_path = ws_path(&p.path);
                            loads.atlases.push(plutonium_game_assets::AtlasLoadRequest {
                                handle,
                                file_path: pan_path,
                                position: Position { x: 0.0, y: 0.0 },
                                tile_size: plutonium_engine::utils::Size {
                                    width: p.tile_width,
                                    height: p.tile_height,
                                },
                            });
                        }
                    }
                }
                // reserve handle and enqueue
                let handle = assets.reserve_handle();
                let loads = game.world.get_resource_mut::<LoadRequests>().unwrap();
                loads.textures.push(TextureLoadRequest {
                    handle,
                    file_path: ws_path("examples/media/player.svg"),
                    position: Position { x: 100.0, y: 100.0 },
                    scale_factor: 1.0,
                });
                unsafe {
                    TEX_HANDLE = Some((handle, uuid::Uuid::nil()));
                }
            }
            if unsafe { BTN_BG_HANDLE.is_none() } {
                let handle = assets.reserve_handle();
                let loads = game.world.get_resource_mut::<LoadRequests>().unwrap();
                loads.textures.push(TextureLoadRequest {
                    handle,
                    file_path: ws_path("examples/media/square.svg"),
                    position: Position {
                        x: start_button.rect.x,
                        y: start_button.rect.y,
                    },
                    scale_factor: 1.0,
                });
                unsafe {
                    BTN_BG_HANDLE = Some((handle, uuid::Uuid::nil()));
                }
            }

            // Update input resource from frame keys + IME text commits
            if let Some(input) = game.world.get_resource_mut::<InputState>() {
                let key_strings = frame.pressed_keys.iter().map(|k| format!("{:?}", k));
                input.update_from_keys(key_strings);
                input.update_mouse(
                    frame.mouse_info.mouse_pos.x,
                    frame.mouse_info.mouse_pos.y,
                    frame.mouse_info.is_lmb_clicked,
                );
            }
            game.world
                .insert_resource(TextCommits(frame.text_commits.clone()));

            // Handle actions
            let action_map = game.world.get_resource::<ActionMap>().unwrap().clone();
            let input_now = game.world.get_resource::<InputState>().unwrap().clone();
            if action_map.action_just_pressed(&input_now, "toggle") {
                is_red = !is_red;
            }
            if action_map.action_just_pressed(&input_now, "start") {
                // Toggle top scene
                let top = game
                    .world
                    .get_resource::<plutonium_game_core::SceneStack>()
                    .and_then(|s| s.top().map(|t| t.to_string()));
                match top.as_deref() {
                    Some("Menu") => {
                        plutonium_game_core::scene_replace(&mut game.world, "Game");
                    }
                    _ => {
                        plutonium_game_core::scene_replace(&mut game.world, "Menu");
                    }
                }
            }

            // Process asset loads
            process_load_requests(&mut game.world, engine);

            // Resolve card texture id once available and store
            if let Some((bh, mut bg_id)) = unsafe { BTN_BG_HANDLE } {
                if bg_id.is_nil() {
                    if let Some(id) = assets.texture_uuid(bh) {
                        bg_id = id;
                        unsafe {
                            BTN_BG_HANDLE = Some((bh, bg_id));
                        }
                    }
                }
                if !bg_id.is_nil() {
                    if game.world.get_resource::<CardTexture>().is_none() {
                        game.world.insert_resource(CardTexture(bg_id));
                    }
                }
            }

            // Initialize deal state on entering Game scene (once)
            let scene_top = game
                .world
                .get_resource::<plutonium_game_core::SceneStack>()
                .and_then(|s| s.top().map(|t| t.to_string()))
                .unwrap_or_else(|| "Menu".to_string());
            if scene_top.as_str() == "Game" {
                let need_init = game.world.get_resource::<DealState>().is_none();
                if need_init {
                    let mut rng = plutonium_game_core::Rng64::seeded(20240721);
                    let cols = 3usize;
                    let rows = 2usize;
                    let start = (60.0f32, 80.0f32);
                    let dx = 40.0f32;
                    let dy = 60.0f32;
                    let mut positions: Vec<(f32, f32)> = Vec::new();
                    for r in 0..rows {
                        for c in 0..cols {
                            positions.push((start.0 + c as f32 * dx, start.1 + r as f32 * dy));
                        }
                    }
                    let mut order: Vec<usize> = (0..positions.len()).collect();
                    // shuffle
                    for i in (1..order.len()).rev() {
                        let j = (rng.next_u64() as usize) % (i + 1);
                        order.swap(i, j);
                    }
                    // spawn cards and build pending
                    let mut ids = Vec::new();
                    let mut pending = Vec::new();
                    let deck_pos = (20.0f32, 20.0f32);
                    for (i, idx) in order.iter().enumerate() {
                        let e = game.world.spawn();
                        game.world.insert_component(
                            e,
                            PositionComp {
                                x: deck_pos.0,
                                y: deck_pos.1,
                            },
                        );
                        ids.push(e);
                        let delay = 0.12f32 * i as f32
                            + ((rng.next_u64() as f32) / (u64::MAX as f32)) * 0.05;
                        let dur = 0.35f32 + ((rng.next_u64() as f32) / (u64::MAX as f32)) * 0.25;
                        pending.push(DealPending {
                            entity: e,
                            delay,
                            to: positions[*idx],
                            duration: dur,
                        });
                    }
                    game.world.insert_resource(Cards { ids });
                    game.world.insert_resource(DealState {
                        pending,
                        started: true,
                        rng,
                    });
                }
            }

            // Drive pending deals: when delay passes, attach TweenPosition to card
            if let Some(ds) = game.world.get_resource_mut::<DealState>() {
                let dt = frame.delta_time;
                for p in ds.pending.iter_mut() {
                    p.delay -= dt;
                }
                // collect ready
                let mut to_start: Vec<DealPending> = Vec::new();
                ds.pending.retain(|p| {
                    if p.delay <= 0.0 {
                        to_start.push(DealPending {
                            entity: p.entity,
                            delay: 0.0,
                            to: p.to,
                            duration: p.duration,
                        });
                        false
                    } else {
                        true
                    }
                });
                for p in to_start {
                    // from current pos component
                    let from = if let Some(pc) =
                        game.world.get_component::<PositionComp>(p.entity).copied()
                    {
                        (pc.x, pc.y)
                    } else {
                        (p.to.0, p.to.1)
                    };
                    game.world
                        .insert_component(p.entity, TweenPosition::new(from, p.to, p.duration));
                }
                // advance card tweens and update base position when finished
                if let Some(cards) = game.world.get_resource::<Cards>() {
                    let card_ids: Vec<_> = cards.ids.iter().copied().collect();
                    for e in card_ids {
                        if let Some(tp) = game.world.get_component_mut::<TweenPosition>(e) {
                            tp.step(frame.delta_time);
                            if tp.finished() {
                                let pos = tp.to;
                                game.world.remove_component::<TweenPosition>(e);
                                if let Some(pc) = game.world.get_component_mut::<PositionComp>(e) {
                                    pc.x = pos.0;
                                    pc.y = pos.1;
                                }
                            }
                        }
                    }
                }
            }

            // Fixed update accumulator
            fixed_accum += frame.delta_time;
            while fixed_accum >= FIXED_STEP {
                // set Time to fixed dt and run fixed schedule
                if let Some(time) = game.world.get_resource_mut::<Time>() {
                    time.delta_seconds = FIXED_STEP;
                }
                game.fixed_update.run(&mut game.world);
                fixed_accum -= FIXED_STEP;
            }

            // Variable update
            game.run_update(frame.delta_time);

            // Handle scene enter/exit events: drive startup schedules, audio, and transitions
            // Apply scene lifecycle and run per-scene updates
            plutonium_game_core::process_scene_events(&mut game.world);

            // Advance transitions: fade alpha down, slide offset animate
            if let Some(fade) = game.world.get_resource_mut::<FadeOverlay>() {
                let rate = 1.5 * frame.delta_time; // seconds to clear
                fade.alpha = (fade.alpha - rate).max(0.0);
            }
            if let Some(off) = game.world.get_resource_mut::<RenderOffset>() {
                // if exiting, slide content to left until -200 px then stop
                if off.dx > -200.0 {
                    off.dx -= 300.0 * frame.delta_time;
                }
            }

            // Build render commands
            let theme_snapshot = game.world.get_resource::<Theme>().cloned();
            let input_snapshot = game.world.get_resource::<InputState>().cloned();
            {
                let cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                cmds.clear();
            }
            let (handle, mut tex_id) = unsafe { TEX_HANDLE.unwrap() };
            if tex_id.is_nil() {
                if let Some(id) = assets.texture_uuid(handle) {
                    tex_id = id;
                    unsafe {
                        TEX_HANDLE = Some((handle, tex_id));
                    }
                }
            }
            // set button background if loaded
            if let Some((bh, mut bg_id)) = unsafe { BTN_BG_HANDLE } {
                if bg_id.is_nil() {
                    if let Some(id) = assets.texture_uuid(bh) {
                        bg_id = id;
                        unsafe {
                            BTN_BG_HANDLE = Some((bh, bg_id));
                        }
                    }
                }
                if !bg_id.is_nil() {
                    start_button.background = Some(bg_id);
                }
            }
            // also try to set from manifest name
            if start_button.background.is_none() {
                if let Some(id) = assets.texture_uuid_by_name("button_bg") {
                    start_button.background = Some(id);
                }
            }
            // Render based on scene top
            let scene_top = game
                .world
                .get_resource::<plutonium_game_core::SceneStack>()
                .and_then(|s| s.top().map(|t| t.to_string()))
                .unwrap_or_else(|| "Menu".to_string());
            match scene_top.as_str() {
                "Menu" => {
                    // Layout: label, button, toggle, slider, text input stacked vertically
                    let stack = StackLayout::new(
                        20.0,
                        20.0,
                        10.0,
                        StackDirection::Vertical,
                        StackCrossAlign::Start,
                    );
                    let rects = stack.layout(&[
                        (320.0, 40.0),
                        (200.0, 36.0),
                        (200.0, 36.0),
                        (200.0, 36.0),
                        (240.0, 36.0),
                    ]);
                    // Label background via 9-slice panel if available (assumes 3x3 tiles in atlas; using tile index order 0..8)
                    if let Some(atlas_id) = assets.atlas_uuid_by_name("panel") {
                        let mut p = DrawParams::default();
                        if let Some(th) = &theme_snapshot {
                            p.tint = th.panel_bg_rgba;
                        }
                        let mut cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        draw_panel_9slice_tiled(
                            &mut cmds,
                            atlas_id,
                            rects[0],
                            (64.0, 64.0),
                            (8.0, 8.0, 8.0, 8.0),
                            p,
                        );
                    }
                    {
                        let cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        cmds.draw_text(
                            "roboto".into(),
                            "Press Enter or Click Button".into(),
                            Position {
                                x: rects[0].x + 8.0,
                                y: rects[0].y + 8.0,
                            },
                            (rects[0].width - 16.0, rects[0].height - 16.0),
                        );
                    }
                    // Button
                    start_button.rect = rects[1];
                    let clicked = if let Some(inp) = &input_snapshot {
                        start_button.update(inp)
                    } else {
                        false
                    };
                    if let Some(th) = &theme_snapshot {
                        let mut cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        start_button.draw(&mut cmds, th);
                    }
                    // Focus ring when button focused
                    {
                        let fm = game.world.get_resource::<FocusManager>().unwrap().clone();
                        if fm.is_focused(0) {
                            if let Some(th) = &theme_snapshot {
                                let mut cmds =
                                    game.world.get_resource_mut::<RenderCommands>().unwrap();
                                if let Some(id) = assets.texture_uuid_by_name("button_bg") {
                                    plutonium_game_ui::draw_focus_ring(
                                        &mut cmds,
                                        start_button.rect,
                                        Some(id),
                                        th,
                                    );
                                }
                            }
                        }
                    }
                    // Toggle
                    sound_toggle.rect = rects[2];
                    let _toggled = if let Some(inp) = &input_snapshot {
                        sound_toggle.update(inp)
                    } else {
                        false
                    };
                    {
                        let mut cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        sound_toggle.draw(&mut cmds);
                    }
                    // Slider
                    volume_slider.rect = rects[3];
                    let _changed = if let Some(inp) = &input_snapshot {
                        volume_slider.update(inp)
                    } else {
                        false
                    };
                    {
                        let mut cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        volume_slider.draw(&mut cmds);
                    }
                    if let Some(audio) = game.world.get_resource::<Audio>() {
                        audio.set_master_volume(volume_slider.value);
                    }
                    // Text input field
                    name_input.rect = rects[4];
                    name_input.update(&game.world);
                    let mut cmds2 = game.world.get_resource_mut::<RenderCommands>().unwrap();
                    if let Some(th) = &theme_snapshot {
                        name_input.draw(&mut cmds2, th);
                    }

                    // Focus + keyboard
                    let fm = game.world.get_resource_mut::<FocusManager>().unwrap();
                    fm.set_count(3); // button, toggle, slider
                    let input = input_snapshot.unwrap_or_default();
                    // Move focus on Tab
                    if input.is_just_pressed("Tab") {
                        fm.next();
                    }
                    // Activate or adjust
                    if fm.is_focused(0) {
                        // button
                        if input.is_just_pressed("Enter") || input.is_just_pressed("Space") {
                            if let Some(scene) = game.world.get_resource_mut::<Scene>() {
                                *scene = Scene::Game;
                            }
                        }
                    } else if fm.is_focused(1) {
                        if input.is_just_pressed("Enter") || input.is_just_pressed("Space") {
                            sound_toggle.on = !sound_toggle.on;
                        }
                    } else if fm.is_focused(2) {
                        if input.is_pressed("ArrowLeft") {
                            volume_slider.value = (volume_slider.value - 0.02).max(0.0);
                        }
                        if input.is_pressed("ArrowRight") {
                            volume_slider.value = (volume_slider.value + 0.02).min(1.0);
                        }
                    }
                    if clicked {
                        plutonium_game_core::scene_replace(&mut game.world, "Game");
                        if let Some(audio) = game.world.get_resource::<Audio>() {
                            audio.play_sfx("assets/sfx/click.wav");
                        }
                    }
                    // (Scene systems disabled in demo to simplify borrows)
                }
                "Game" => {
                    let player_e = game.world.get_resource::<Player>().unwrap().0;
                    let (params, draw_pos) = {
                        let mut params = DrawParams::default();
                        if let Some(tween) = game.world.get_component_mut::<TweenScale>(player_e) {
                            tween.step(frame.delta_time);
                            params.scale = tween.current();
                            if tween.finished() {
                                *tween = TweenScale::new(tween.to, tween.from, tween.duration);
                            }
                        }
                        if let Some(alpha) = game.world.get_component_mut::<TweenAlpha>(player_e) {
                            alpha.step(frame.delta_time);
                            let a = ease_value(
                                Ease::QuadOut,
                                (alpha.t / alpha.duration).clamp(0.0, 1.0),
                            );
                            params.tint = [1.0, 1.0, 1.0, a];
                            if alpha.finished() {
                                *alpha = TweenAlpha::new(alpha.to, alpha.from, alpha.duration);
                            }
                        }
                        if let Some(tp) = game.world.get_component_mut::<TweenPosition>(player_e) {
                            tp.step(frame.delta_time);
                            let p = tp.current();
                            if tp.finished() {
                                *tp = TweenPosition::new(tp.to, tp.from, tp.duration);
                            }
                            (params, Some(Position { x: p.0, y: p.1 }))
                        } else {
                            let p = game
                                .world
                                .get_component::<PositionComp>(player_e)
                                .copied()
                                .map(|pc| Position { x: pc.x, y: pc.y });
                            (params, p)
                        }
                    };
                    if let Some(pos) = draw_pos {
                        let cmds3 = game.world.get_resource_mut::<RenderCommands>().unwrap();
                        cmds3.draw_sprite(tex_id, pos, params);
                    }
                    let cmds4 = game.world.get_resource_mut::<RenderCommands>().unwrap();
                    cmds4.draw_text(
                        "roboto".into(),
                        "Game Running".into(),
                        Position { x: 20.0, y: 20.0 },
                        (200.0, 40.0),
                    );

                    // Draw dealt cards
                    let cards_resource = game.world.get_resource::<Cards>();
                    let card_texture_resource = game.world.get_resource::<CardTexture>();

                    if let (Some(cards), Some(CardTexture(card_tex))) =
                        (cards_resource, card_texture_resource)
                    {
                        // Clone the data we need to avoid borrowing issues
                        let card_ids: Vec<_> = cards.ids.iter().copied().collect();
                        let card_tex_clone = *card_tex;

                        // Collect all card data first to avoid borrowing issues
                        let mut card_data = Vec::new();
                        for e in card_ids {
                            let pos_xy = if let Some(tp) =
                                game.world.get_component::<TweenPosition>(e)
                            {
                                Some(tp.current())
                            } else if let Some(pc) = game.world.get_component::<PositionComp>(e) {
                                Some((pc.x, pc.y))
                            } else {
                                None
                            };
                            if let Some(pos) = pos_xy {
                                card_data.push((pos, card_tex_clone));
                            }
                        }

                        // Now render with the collected data
                        if !card_data.is_empty() {
                            let cmds = game.world.get_resource_mut::<RenderCommands>().unwrap();
                            for ((x, y), card_tex) in card_data {
                                let mut p = DrawParams::default();
                                p.scale = 1.0;
                                cmds.draw_sprite(card_tex, Position { x, y }, p);
                            }
                        }
                    }
                }
                _ => {}
            }

            // Submit draw commands to engine
            engine.begin_frame();
            let _tint = if is_red {
                [1.0, 0.5, 0.5, 1.0]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            // ensure a font is loaded once (required before queueing text)
            static mut FONT_LOADED: bool = false;
            if unsafe { !FONT_LOADED } {
                let _ = engine.load_font(&ws_path("examples/media/roboto.ttf"), 16.0, "roboto");
                unsafe {
                    FONT_LOADED = true;
                }
            }
            // Use the render system that reads RenderCommands from world
            // Optional per-frame metrics
            if let Some(m) = game.world.get_resource::<Metrics>().copied() {
                if m.enabled {
                    if let Some(cmds) = game.world.get_resource::<RenderCommands>() {
                        println!(
                            "metrics: dt_ms={:.2} sprites={} texts={} tiles={}",
                            frame.delta_time * 1000.0,
                            cmds.sprites.len(),
                            cmds.texts.len(),
                            cmds.atlas_tiles.len()
                        );
                    }
                }
            }
            render_system(&mut game.world, engine);
        },
    );
}
