use std::collections::HashMap;

use uuid::Uuid;

use crate::texture_atlas::TextureAtlas;
use crate::texture_svg::TextureSVG;
// use crate::PlutoniumEngine; // not needed right now

// Expose the queued items to the renderer backend
// Reserved for future use

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
        // Engine currently issues the actual bind groups during queueing;
        // this backend seam is reserved for future full refactor.
        // No-op here to keep behavior unchanged while indices are introduced in the queue.
    }
}


