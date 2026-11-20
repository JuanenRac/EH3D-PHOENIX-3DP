//! PHOENIX-3DP viewer: the model of the 3D viewer, with no graphics code in it. An orbit camera with perspective (turn, zoom, slide, fit to a box, project a point, a ray through a
//! pixel), the scene on the plate (objects as boxes, a click that picks the nearest, a box dragged over the screen), the range of layers of a plan and the lines of those layers as
//! arrays of numbers for a graphics card. All of it is plain arithmetic and is tested without a screen.
//!
//! The part that draws with wgpu is in the module `gpu`, behind the feature `gui`. It is written from the documentation of wgpu and has not been compiled or run on a screen, so the
//! feature is off by default and nothing of the tests depends on it.
#![forbid(unsafe_code)]

pub mod camera;
#[cfg(feature = "gui")]
pub mod gpu;
pub mod layers;
pub mod math;
pub mod scene;

pub use camera::{OrbitCamera, Ray, Viewport};
pub use layers::{lines_of, plate_lines, travel_lines, KindFilter, LayerRange, LineBatch};
pub use scene::{BoxMode, Plate, Scene, SceneError, SceneObject, ScreenRect};

/// The shader that draws the lines of a [`LineBatch`]: a matrix from the world to the clip space, the position and the colour of each vertex. It is plain text of the language of
/// wgpu (WGSL); the pipeline in `gpu` is built with it.
pub const LINE_SHADER: &str = "\
struct View {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) colour: vec3<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) colour: vec3<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = view.view_proj * vec4<f32>(v.position, 1.0);
    out.colour = v.colour;
    return out;
}

@fragment
fn fs_main(v: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(v.colour, 1.0);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_has_the_two_entry_points_and_the_layout_of_a_vertex_of_the_batch() {
        assert!(LINE_SHADER.contains("fn vs_main") && LINE_SHADER.contains("fn fs_main"));
        assert!(LINE_SHADER.contains("@location(0) position: vec3<f32>"));
        assert!(LINE_SHADER.contains("@location(1) colour: vec3<f32>"));
        assert!(LINE_SHADER.contains("@group(0) @binding(0)"));
        // a vertex of the batch is six numbers of four bytes: position then colour
        assert_eq!(layers::FLOATS_PER_VERTEX * 4, 24);
    }

    #[test]
    fn what_a_program_needs_is_reachable_from_the_top() {
        let mut cam = OrbitCamera::default();
        let vp = Viewport::new(640.0, 480.0).unwrap();
        let mut scene = Scene::new(220.0, 220.0).unwrap();
        scene
            .add(SceneObject {
                id: 1,
                name: "cube".into(),
                lo: [100.0, 100.0, 0.0],
                hi: [120.0, 120.0, 20.0],
            })
            .unwrap();
        let (lo, hi) = scene.bounds();
        cam.fit_to_box(lo, hi, vp);
        let at = cam.project([110.0, 110.0, 10.0], vp).unwrap();
        assert_eq!(scene.pick(&cam, vp, at), Some(1));
    }
}
