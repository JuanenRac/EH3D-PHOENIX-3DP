//! The drawing of the lines of the viewer with wgpu (feature `gui`). A pipeline of lines over the vertices of a [`LineBatch`] (position and colour), a uniform with the matrix from the
//! world to the clip space, and the calls that fill the buffers and draw. It makes no window and no surface: the program that has a surface gives the device, the queue and the
//! format of its target, and a render pass to draw into.
//!
//! This module was written from the documentation of wgpu 26 and has not been compiled or run: the tests of the crate do not build it, and the first time it is built is the
//! first time anything in it is checked.

use crate::layers::{LineBatch, FLOATS_PER_VERTEX};
use crate::LINE_SHADER;
use wgpu::util::DeviceExt;

const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

/// The pipeline, the matrix and the vertices of one batch.
pub struct LineRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
    vertex_count: u32,
}

impl LineRenderer {
    /// Builds the pipeline for a target of this format.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> LineRenderer {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("phoenix-3dp lines"),
            source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("phoenix-3dp view"),
            contents: &[0u8; 64],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("phoenix-3dp view layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("phoenix-3dp view group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("phoenix-3dp lines layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("phoenix-3dp lines"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: (FLOATS_PER_VERTEX * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRIBUTES,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        LineRenderer {
            pipeline,
            uniform,
            bind_group,
            vertices: None,
            vertex_count: 0,
        }
    }

    /// Puts a new batch of lines and the matrix of the camera (16 numbers, column by column, from `OrbitCamera::view_projection_f32`) into the buffers.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &LineBatch,
        view_projection: &[f32; 16],
    ) {
        let matrix: Vec<u8> = view_projection
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        queue.write_buffer(&self.uniform, 0, &matrix);
        if batch.is_empty() {
            self.vertices = None;
            self.vertex_count = 0;
            return;
        }
        self.vertices = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("phoenix-3dp vertices"),
                contents: &batch.to_bytes(),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.vertex_count = batch.vertex_count() as u32;
    }

    /// Draws the batch into a render pass.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(buffer) = &self.vertices else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}
