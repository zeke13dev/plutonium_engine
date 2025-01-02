use crate::utils::{Position, Rectangle};
use crate::PlutoObject;
use crate::PlutoniumEngine;
use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;
use uuid::Uuid;

#[derive(Clone)]
pub enum ShapeType {
    Rectangle,
    Circle,
    Polygon(u32),
}

pub struct ShapeInternal {
    id: Uuid,
    texture_id: Uuid,
    bounds: Rectangle,
    position: Position,
    fill: String,
    outline: String,
    stroke: f32,
    shape_type: ShapeType,
}

impl ShapeInternal {
    pub fn set_ids(&mut self, id: Uuid, texture_id: Uuid) {
        self.id = id;
        self.texture_id = texture_id;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        texture_id: Uuid,
        bounds: Rectangle,
        position: Position,
        fill: String,
        outline: String,
        stroke: f32,
        shape_type: ShapeType,
    ) -> Self {
        Self {
            id,
            texture_id,
            bounds,
            position,
            fill,
            outline,
            stroke,
            shape_type,
        }
    }

    pub fn generate_svg_data(&self) -> String {
        let width = self.bounds.width;
        let height = self.bounds.height;

        match &self.shape_type {
            ShapeType::Rectangle => format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
                    <rect width="{}" height="{}" 
                        fill="{}" 
                        stroke="{}" 
                        stroke-width="{}"/>
                </svg>"#,
                width, height, width, height, self.fill, self.outline, self.stroke
            ),

            ShapeType::Circle => format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
                    <circle cx="{}" cy="{}" r="{}"
                        fill="{}"
                        stroke="{}"
                        stroke-width="{}"/>
                </svg>"#,
                width,
                height,
                width / 2.0,
                height / 2.0,
                width / 2.0,
                self.fill,
                self.outline,
                self.stroke
            ),

            ShapeType::Polygon(points) => {
                let center_x = width / 2.0;
                let center_y = height / 2.0;
                let radius = width / 2.0;
                let mut path = String::new();

                for i in 0..*points {
                    let angle = (i as f32) * 2.0 * PI / (*points as f32);
                    let x = center_x + radius * angle.cos();
                    let y = center_y + radius * angle.sin();

                    if i == 0 {
                        path.push_str(&format!("M {} {}", x, y));
                    } else {
                        path.push_str(&format!(" L {} {}", x, y));
                    }
                }
                path.push_str(" Z");

                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
                        <path d="{}"
                            fill="{}"
                            stroke="{}"
                            stroke-width="{}"/>
                    </svg>"#,
                    width, height, path, self.fill, self.outline, self.stroke
                )
            }
        }
    }
}

pub struct Shape {
    internal: Rc<RefCell<ShapeInternal>>,
}

impl Shape {
    pub fn new(internal: Rc<RefCell<ShapeInternal>>) -> Self {
        Self { internal }
    }

    // Wrapper functions to match other objects
    pub fn get_id(&self) -> Uuid {
        self.internal.borrow().get_id()
    }

    pub fn texture_key(&self) -> Uuid {
        self.internal.borrow().texture_key()
    }

    pub fn dimensions(&self) -> Rectangle {
        self.internal.borrow().dimensions()
    }

    pub fn pos(&self) -> Position {
        self.internal.borrow().pos()
    }

    pub fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.internal.borrow_mut().set_dimensions(new_dimensions);
    }

    pub fn set_pos(&mut self, new_pos: Position) {
        self.internal.borrow_mut().set_pos(new_pos);
    }

    // Additional shape-specific getters if needed
    pub fn fill(&self) -> String {
        self.internal.borrow().fill.clone()
    }

    pub fn outline(&self) -> String {
        self.internal.borrow().outline.clone()
    }

    pub fn stroke(&self) -> f32 {
        self.internal.borrow().stroke
    }

    // You might want to add setters for these as well
    pub fn set_fill(&mut self, fill: String) {
        self.internal.borrow_mut().fill = fill;
    }

    pub fn set_outline(&mut self, outline: String) {
        self.internal.borrow_mut().outline = outline;
    }

    pub fn set_stroke(&mut self, stroke: f32) {
        self.internal.borrow_mut().stroke = stroke;
    }

    pub fn render(&self, engine: &mut PlutoniumEngine) {
        self.internal.borrow().render(engine);
    }
}
// Move PlutoObject implementation to ShapeInternal
impl PlutoObject for ShapeInternal {
    fn texture_key(&self) -> Uuid {
        self.texture_id
    }

    fn get_id(&self) -> Uuid {
        self.id
    }

    fn dimensions(&self) -> Rectangle {
        self.bounds
    }

    fn pos(&self) -> Position {
        self.position
    }

    fn set_dimensions(&mut self, new_dimensions: Rectangle) {
        self.bounds = new_dimensions;
    }

    fn set_pos(&mut self, new_pos: Position) {
        self.position = new_pos;
    }
}
