struct Uniforms { view_projection: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Uniforms;
@group(1) @binding(0) var atlas: texture_2d_array<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
struct VertexInput { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) texture_layer: u32, @location(3) color: vec3<f32> };
struct VertexOutput { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) texture_layer: u32, @location(2) color: vec3<f32> };
@vertex fn vs_main(input: VertexInput) -> VertexOutput { var out: VertexOutput; out.position = camera.view_projection * vec4<f32>(input.position, 1.0); out.uv = input.uv; out.texture_layer = input.texture_layer; out.color = input.color; return out; }
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> { if (input.texture_layer < 64u) { return textureSample(atlas, atlas_sampler, input.uv, i32(input.texture_layer)); } return vec4<f32>(input.color, 1.0); }
