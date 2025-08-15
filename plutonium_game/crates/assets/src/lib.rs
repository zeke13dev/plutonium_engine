#![forbid(unsafe_code)]

use plutonium_game_core::World;
use serde::de::Error as SerdeDeError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::time::SystemTime;
use toml::de::Error as TomlError;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Handle(pub u64);

#[derive(Default)]
pub struct AssetsRegistry {
    next: u64,
    texture_handles: HashMap<Handle, Uuid>,
    name_to_handle: HashMap<String, Handle>,
    atlas_handles: HashMap<Handle, Uuid>,
    name_to_atlas_handle: HashMap<String, Handle>,
    // Ref counts (by Handle)
    refs_tex: HashMap<Handle, usize>,
    refs_atlas: HashMap<Handle, usize>,
    // Hot-reload cache of file mtimes (dev only)
    file_mtimes: HashMap<String, SystemTime>,
}

impl AssetsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_texture_svg(
        &mut self,
        engine: &mut plutonium_engine::PlutoniumEngine,
        file_path: &str,
        position: plutonium_engine::utils::Position,
        scale_factor: f32,
    ) -> (Handle, plutonium_engine::utils::Rectangle) {
        let (uuid, dims) = engine.create_texture_svg(file_path, position, scale_factor);
        let handle = self.reserve_handle();
        self.texture_handles.insert(handle, uuid);
        self.refs_tex.insert(handle, 1);
        self.file_mtimes.insert(
            file_path.to_string(),
            std::fs::metadata(file_path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH),
        );
        (handle, dims)
    }

    pub fn texture_uuid(&self, handle: Handle) -> Option<Uuid> {
        self.texture_handles.get(&handle).copied()
    }

    pub fn reserve_handle(&mut self) -> Handle {
        self.next += 1;
        Handle(self.next)
    }

    pub fn set_texture_uuid(&mut self, handle: Handle, uuid: Uuid) {
        self.texture_handles.insert(handle, uuid);
        *self.refs_tex.entry(handle).or_insert(0) += 1;
    }

    pub fn set_named_handle(&mut self, name: &str, handle: Handle) {
        self.name_to_handle.insert(name.to_string(), handle);
    }

    pub fn texture_uuid_by_name(&self, name: &str) -> Option<Uuid> {
        let h = self.name_to_handle.get(name)?;
        self.texture_handles.get(h).copied()
    }

    pub fn set_named_atlas_handle(&mut self, name: &str, handle: Handle) {
        self.name_to_atlas_handle.insert(name.to_string(), handle);
    }
    pub fn set_atlas_uuid(&mut self, handle: Handle, uuid: Uuid) {
        self.atlas_handles.insert(handle, uuid);
        *self.refs_atlas.entry(handle).or_insert(0) += 1;
    }
    pub fn atlas_uuid(&self, handle: Handle) -> Option<Uuid> {
        self.atlas_handles.get(&handle).copied()
    }
    pub fn atlas_uuid_by_name(&self, name: &str) -> Option<Uuid> {
        let h = self.name_to_atlas_handle.get(name)?;
        self.atlas_handles.get(h).copied()
    }
}

#[derive(Debug, Clone)]
pub struct TextureLoadRequest {
    pub handle: Handle,
    pub file_path: String,
    pub position: plutonium_engine::utils::Position,
    pub scale_factor: f32,
}

#[derive(Default)]
pub struct LoadRequests {
    pub textures: Vec<TextureLoadRequest>,
    pub atlases: Vec<AtlasLoadRequest>,
}

pub fn process_load_requests(world: &mut World, engine: &mut plutonium_engine::PlutoniumEngine) {
    // Move out requests first to avoid overlapping mutable borrows
    let (pending_textures, pending_atlases) = {
        let Some(loads) = world.get_resource_mut::<LoadRequests>() else {
            return;
        };
        (
            std::mem::take(&mut loads.textures),
            std::mem::take(&mut loads.atlases),
        )
    };
    // Apply texture loads
    if !pending_textures.is_empty() {
        if let Some(registry) = world.get_resource_mut::<AssetsRegistry>() {
            for req in pending_textures {
                let (uuid, _dims) =
                    engine.create_texture_svg(&req.file_path, req.position, req.scale_factor);
                registry.set_texture_uuid(req.handle, uuid);
            }
        }
    }
    // Apply atlas loads
    if !pending_atlases.is_empty() {
        if let Some(registry) = world.get_resource_mut::<AssetsRegistry>() {
            for req in pending_atlases {
                let (uuid, _rect) =
                    engine.create_texture_atlas(&req.file_path, req.position, req.tile_size);
                registry.set_atlas_uuid(req.handle, uuid);
            }
        }
    }
}

impl AssetsRegistry {
    pub fn retain_texture(&mut self, handle: Handle) {
        *self.refs_tex.entry(handle).or_insert(0) += 1;
    }
    pub fn release_texture(
        &mut self,
        handle: Handle,
        engine: &mut plutonium_engine::PlutoniumEngine,
    ) {
        if let Some(r) = self.refs_tex.get_mut(&handle) {
            if *r > 0 {
                *r -= 1;
            }
            if *r == 0 {
                if let Some(uuid) = self.texture_handles.remove(&handle) {
                    let _ = engine.unload_texture(&uuid);
                }
            }
        }
    }
    pub fn retain_atlas(&mut self, handle: Handle) {
        *self.refs_atlas.entry(handle).or_insert(0) += 1;
    }
    pub fn release_atlas(
        &mut self,
        handle: Handle,
        engine: &mut plutonium_engine::PlutoniumEngine,
    ) {
        if let Some(r) = self.refs_atlas.get_mut(&handle) {
            if *r > 0 {
                *r -= 1;
            }
            if *r == 0 {
                if let Some(uuid) = self.atlas_handles.remove(&handle) {
                    let _ = engine.unload_atlas(&uuid);
                }
            }
        }
    }

    #[cfg(feature = "dev-hot-reload")]
    pub fn poll_hot_reload(&mut self, engine: &mut plutonium_engine::PlutoniumEngine) {
        // Re-create textures/atlases whose source files changed
        for (name, handle) in self.name_to_handle.clone() {
            if let Some(uuid) = self.texture_handles.get(&handle).copied() {
                if let Some(path) = self.find_path_for_handle(&name) {
                    let mtime = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    if self
                        .file_mtimes
                        .get(&path)
                        .copied()
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                        < mtime
                    {
                        // reload
                        let _ = engine.unload_texture(&uuid);
                        let (new_uuid, _dims) = engine.create_texture_svg(
                            &path,
                            plutonium_engine::utils::Position { x: 0.0, y: 0.0 },
                            1.0,
                        );
                        self.texture_handles.insert(handle, new_uuid);
                        self.file_mtimes.insert(path.clone(), mtime);
                    }
                }
            }
        }
    }

    fn find_path_for_handle(&self, name: &str) -> Option<String> {
        // For now, assume the manifest-specified name maps to a relative path identical to original config lookup
        self.file_mtimes.keys().find(|p| p.contains(name)).cloned()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfig {
    pub primary_text_rgba: [f32; 4],
    pub button_bg_rgba: [f32; 4],
    pub button_bg_hover_rgba: [f32; 4],
}

#[derive(Debug, Clone, Deserialize)]
pub struct FontConfig {
    pub path: String,
    pub key: String,
    pub size: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetManifest {
    pub theme: ThemeConfig,
    pub fonts: Vec<FontConfig>,
    #[serde(default)]
    pub textures: Vec<TextureConfig>,
    #[serde(default)]
    pub panels: Vec<PanelConfig>,
}

pub fn load_manifest(path: &str) -> Result<AssetManifest, TomlError> {
    let data = read_to_string(path).map_err(|e| TomlError::custom(e.to_string()))?;
    toml::from_str(&data)
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextureConfig {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PanelConfig {
    pub name: String,
    pub path: String,
    pub tile_width: f32,
    pub tile_height: f32,
    pub inset_left: f32,
    pub inset_right: f32,
    pub inset_top: f32,
    pub inset_bottom: f32,
}

#[derive(Debug, Clone)]
pub struct AtlasLoadRequest {
    pub handle: Handle,
    pub file_path: String,
    pub position: plutonium_engine::utils::Position,
    pub tile_size: plutonium_engine::utils::Size,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_round_trip() {
        // Minimal valid manifest
        let toml = r#"
            [theme]
            primary_text_rgba = [1.0, 1.0, 1.0, 1.0]
            button_bg_rgba = [0.2, 0.2, 0.25, 1.0]
            button_bg_hover_rgba = [0.3, 0.3, 0.35, 1.0]

            [[fonts]]
            path = "examples/media/roboto.ttf"
            key = "roboto"
            size = 16.0

            [[textures]]
            name = "button_bg"
            path = "examples/media/square.svg"
        "#;
        let manifest: AssetManifest = toml::from_str(toml).expect("parse");
        assert_eq!(manifest.fonts.len(), 1);
        assert_eq!(manifest.textures.len(), 1);
        assert_eq!(manifest.theme.primary_text_rgba[0], 1.0);
    }
}
