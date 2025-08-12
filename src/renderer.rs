use std::collections::HashMap;

use uuid::Uuid;

use crate::texture_atlas::TextureAtlas;
use crate::texture_svg::TextureSVG;
// use crate::PlutoniumEngine; // not needed right now

// Expose the queued items to the renderer backend
pub(crate) enum RenderItemRef<'a> {
    Texture {
        texture: &'a TextureSVG,
        transform_bind_group: &'a wgpu::BindGroup,
    },
    AtlasTile {
        atlas: &'a TextureAtlas,
        transform_bind_group: &'a wgpu::BindGroup,
        tile_index: usize,
    },
}

pub trait Renderer {
    fn submit<'a>(
        &mut self,
        rpass: &mut wgpu::RenderPass<'a>,
        pipeline: &'a wgpu::RenderPipeline,
        items: &'a [crate::QueuedItem],
        texture_map: &'a HashMap<Uuid, TextureSVG>,
        atlas_map: &'a HashMap<Uuid, TextureAtlas>,
    );
}

pub struct WgpuRenderer;

impl WgpuRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Renderer for WgpuRenderer {
    fn submit<'a>(
        &mut self,
        rpass: &mut wgpu::RenderPass<'a>,
        pipeline: &'a wgpu::RenderPipeline,
        items: &'a [crate::QueuedItem],
        texture_map: &'a HashMap<Uuid, TextureSVG>,
        atlas_map: &'a HashMap<Uuid, TextureAtlas>,
    ) {
        for queued in items {
            match &queued.item {
                crate::RenderItem::Texture {
                    texture_key,
                    transform_bind_group,
                } => {
                    if let Some(texture) = texture_map.get(texture_key) {
                        texture.render(rpass, pipeline, transform_bind_group);
                    }
                }
                crate::RenderItem::AtlasTile {
                    texture_key,
                    transform_bind_group,
                    tile_index,
                } => {
                    if let Some(atlas) = atlas_map.get(texture_key) {
                        atlas.render_tile(rpass, pipeline, *tile_index, transform_bind_group);
                    }
                }
            }
        }
    }
}


