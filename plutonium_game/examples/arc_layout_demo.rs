//! Example demonstrating ArcLayout usage for distributing items along elliptical arcs
//!
//! This example shows how to use ArcLayout to:
//! 1. Create circular arcs using the convenience method
//! 2. Create elliptical arcs with different horizontal/vertical radii
//! 3. Create wide, flat curves (large radius_x, small radius_y)
//! 4. Create tall, steep curves (small radius_x, large radius_y)
//! 5. Use layout_with_rotation for card fanning with proper ellipse tangent rotation

use plutonium_engine::{
    app::{run_app, FrameContext, WindowConfig},
    PlutoniumEngine,
};
use plutonium_game_ui::{ArcLayout, DrawParams, RenderCommands};
use std::f32::consts::PI;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmds = RenderCommands::default();
    let mut card_texture: Option<uuid::Uuid> = None;

    run_app(
        WindowConfig {
            title: "Arc Layout Demo".to_string(),
            width: 800,
            height: 600,
        },
        move |engine: &mut PlutoniumEngine, _frame: &FrameContext| {
            // Load a card texture on first frame
            if card_texture.is_none() {
                // Use a simple square as a card placeholder
                // Path relative to workspace root
                if let Ok((tex_id, _)) = engine.create_texture_svg(
                    "../../examples/media/square.svg",
                    plutonium_engine::utils::Position { x: 0.0, y: 0.0 },
                    1.0,
                ) {
                    card_texture = Some(tex_id);
                }
            }

            cmds.clear();

            // Example 1: Circular arc (using convenience method)
            let arc1 = ArcLayout::circular(
                200.0,     // center_x
                150.0,     // center_y
                100.0,     // radius
                -PI / 4.0, // start_angle
                PI / 4.0,  // end_angle
            );

            let card_size = (40.0, 60.0);
            let card_sizes = vec![card_size; 7];
            let card_rects = arc1.layout(&card_sizes);

            if let Some(tex_id) = card_texture {
                for rect in &card_rects {
                    cmds.draw_sprite(
                        tex_id,
                        rect.pos(),
                        DrawParams {
                            z: 0,
                            rotation: 0.0,
                            scale: 1.0,
                            tint: [0.8, 0.3, 0.3, 1.0], // Red - circular
                        },
                    );
                }
            }

            // Example 2: Wide elliptical arc (flatter curve)
            let arc2 = ArcLayout::new(
                400.0,  // center_x
                150.0,  // center_y
                200.0,  // radius_x (wide)
                80.0,   // radius_y (shallow)
                -PI / 4.0,
                PI / 4.0,
            );

            let arc2_rects = arc2.layout(&card_sizes);

            if let Some(tex_id) = card_texture {
                for rect in &arc2_rects {
                    cmds.draw_sprite(
                        tex_id,
                        rect.pos(),
                        DrawParams {
                            z: 0,
                            rotation: 0.0,
                            scale: 1.0,
                            tint: [0.3, 0.8, 0.3, 1.0], // Green - wide ellipse
                        },
                    );
                }
            }

            // Example 3: Tall elliptical arc (more curved vertically)
            let arc3 = ArcLayout::new(
                600.0,  // center_x
                150.0,  // center_y
                80.0,   // radius_x (narrow)
                120.0,  // radius_y (tall)
                -PI / 4.0,
                PI / 4.0,
            );

            let arc3_rects = arc3.layout(&card_sizes);

            if let Some(tex_id) = card_texture {
                for rect in &arc3_rects {
                    cmds.draw_sprite(
                        tex_id,
                        rect.pos(),
                        DrawParams {
                            z: 0,
                            rotation: 0.0,
                            scale: 1.0,
                            tint: [0.3, 0.3, 0.8, 1.0], // Blue - tall ellipse
                        },
                    );
                }
            }

            // Example 4: Elliptical arc with rotation (card fanning)
            let arc4 = ArcLayout::new(
                400.0,     // center_x
                450.0,     // center_y
                220.0,     // radius_x (wide)
                140.0,     // radius_y
                -PI / 3.0, // start_angle
                PI / 3.0,  // end_angle
            );

            let rotated_positions = arc4.layout_with_rotation(9);

            if let Some(tex_id) = card_texture {
                for (pos, rotation) in rotated_positions {
                    cmds.draw_sprite(
                        tex_id,
                        plutonium_engine::utils::Position {
                            x: pos.x - card_size.0 * 0.5,
                            y: pos.y - card_size.1 * 0.5,
                        },
                        DrawParams {
                            z: 1,
                            rotation,
                            scale: 1.0,
                            tint: [0.8, 0.8, 0.3, 1.0], // Yellow - rotated ellipse
                        },
                    );
                }
            }

            // Submit render commands
            engine.begin_frame();
            plutonium_game_ui::submit_render_commands(engine, &cmds, [1.0, 1.0, 1.0, 1.0]);
            engine.end_frame().unwrap();
        },
    )?;

    Ok(())
}

