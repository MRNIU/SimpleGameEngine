// Copyright The SimpleGameEngine Contributors
//
// wgpu viewport mesh and composite shaders.

struct CameraUniform {
  view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var viewport_color: texture_2d<f32>;

@group(0) @binding(2)
var viewport_sampler: sampler;

struct VertexInput {
  @location(0) position: vec3<f32>,
  @location(1) color: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_mesh(input: VertexInput) -> VertexOutput {
  var output: VertexOutput;
  output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
  output.color = input.color;
  return output;
}

@fragment
fn fs_mesh(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}

struct CompositeOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_composite(@builtin(vertex_index) index: u32) -> CompositeOutput {
  let positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0),
  );
  let position = positions[index];
  var output: CompositeOutput;
  output.clip_position = vec4<f32>(position, 0.0, 1.0);
  output.uv = vec2<f32>(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
  return output;
}

@fragment
fn fs_composite(input: CompositeOutput) -> @location(0) vec4<f32> {
  return textureSample(viewport_color, viewport_sampler, input.uv);
}
