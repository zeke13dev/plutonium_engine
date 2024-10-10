use crate::texture_svg::TextureSVG;
use crate::utils::{Position, Size};
use wgpu::{Device, Queue};

pub struct Text {
    texture_svg: TextureSVG,
}

impl Text {
    /// Creates a new Text instance that converts text to an SVG texture.
    pub fn new(
        device: &Device,
        queue: &Queue,
        text: &str,
        font_size: f32,
        position: Position,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Option<Self> {
        let svg_data = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="500" height="200">
                    <text x="0" y="50" font-family="Verdana" font-size="{}" fill="black">{}</text>
                </svg>"#,
            font_size, text
        );

        let file_path = "temp_text.svg";
        std::fs::write(file_path, svg_data).expect("Unable to write file");

        let texture_svg = TextureSVG::new(
            device,
            queue,
            file_path,
            texture_bind_group_layout,
            transform_bind_group_layout,
            position,
            1.0,
            None,
        )?;

        Some(Self { texture_svg })
    }

    /// Renders the text using the underlying TextureSVG.
    pub fn render<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        render_pipeline: &'a wgpu::RenderPipeline,
    ) {
        self.texture_svg.render(rpass, render_pipeline, None, None);
    }

    /// Updates the position of the text.
    pub fn set_position(
        &mut self,
        device: &Device,
        queue: &Queue,
        position: Position,
        viewport_size: Size,
        camera_position: Position,
    ) {
        self.texture_svg
            .set_position(device, queue, position, viewport_size, camera_position);
    }

    /// Additional methods for Text, extending TextureSVG functionality
    pub fn get_size(&self) -> Size {
        self.texture_svg.size()
    }

    pub fn get_position(&self) -> Position {
        self.texture_svg.pos()
    }
}
