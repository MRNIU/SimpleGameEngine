// Copyright The SimpleGameEngine Contributors

struct FrameUniform {
  view_projection: mat4x4<f32>,
  light_direction_intensity: vec4<f32>,
  light_color: vec4<f32>,
  render_settings: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

@group(1) @binding(0)
var color_texture: texture_2d<f32>;

@group(1) @binding(1)
var color_sampler: sampler;

struct MeshInput {
  @location(0) position: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) model_0: vec4<f32>,
  @location(3) model_1: vec4<f32>,
  @location(4) model_2: vec4<f32>,
  @location(5) model_3: vec4<f32>,
  @location(6) color: vec4<f32>,
  @location(7) normal_0: vec4<f32>,
  @location(8) normal_1: vec4<f32>,
  @location(9) normal_2: vec4<f32>,
  @location(11) uv: vec2<f32>,
  @location(12) has_texture: vec4<f32>,
};

struct MeshOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) normal: vec3<f32>,
  @location(1) color: vec4<f32>,
  @location(2) uv: vec2<f32>,
  @location(3) has_texture: f32,
};

@vertex
fn vs_mesh(input: MeshInput) -> MeshOutput {
  let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
  var output: MeshOutput;
  output.clip_position = frame.view_projection * model * vec4<f32>(input.position, 1.0);
  let normal_matrix = mat3x3<f32>(input.normal_0.xyz, input.normal_1.xyz, input.normal_2.xyz);
  output.normal = normalize(normal_matrix * input.normal);
  output.color = input.color;
  output.uv = input.uv;
  output.has_texture = input.has_texture.x;
  return output;
}

@fragment
fn fs_mesh(input: MeshOutput) -> @location(0) vec4<f32> {
  var material = input.color;
  if input.has_texture > 0.5 {
    material = textureSample(color_texture, color_sampler, input.uv) * input.color;
  }
  if frame.render_settings.x == 1.0 || frame.light_direction_intensity.w < 0.0 {
    return material;
  }
  let light = max(dot(input.normal, -frame.light_direction_intensity.xyz), 0.0);
  let strength = 0.15 + light * frame.light_direction_intensity.w;
  return vec4<f32>(material.rgb * frame.light_color.rgb * strength, material.a);
}

struct WireInput {
  @location(0) position: vec3<f32>,
  @location(2) model_0: vec4<f32>,
  @location(3) model_1: vec4<f32>,
  @location(4) model_2: vec4<f32>,
  @location(5) model_3: vec4<f32>,
  @location(10) barycentric: vec3<f32>,
};

struct WireOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) @interpolate(linear) barycentric: vec3<f32>,
};

@vertex
fn vs_wire(input: WireInput) -> WireOutput {
  let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
  var output: WireOutput;
  output.clip_position = frame.view_projection * model * vec4<f32>(input.position, 1.0);
  output.barycentric = input.barycentric;
  return output;
}

@fragment
fn fs_wire(input: WireOutput) -> @location(0) vec4<f32> {
  let derivatives = max(fwidth(input.barycentric), vec3<f32>(1.0e-6));
  let edge_distance = min(
    min(input.barycentric.x / derivatives.x, input.barycentric.y / derivatives.y),
    input.barycentric.z / derivatives.z,
  );
  if edge_distance > frame.render_settings.y {
    discard;
  }
  if frame.render_settings.x == 2.0 {
    return vec4<f32>(0.75, 0.80, 0.90, 1.0);
  }
  return vec4<f32>(0.02, 0.02, 0.02, 1.0);
}

struct CompositeOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@group(0) @binding(1)
var offscreen_color: texture_2d<f32>;

@group(0) @binding(2)
var offscreen_sampler: sampler;

@vertex
fn vs_composite(@builtin(vertex_index) index: u32) -> CompositeOutput {
  let positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0)
  );
  let position = positions[index];
  var output: CompositeOutput;
  output.clip_position = vec4<f32>(position, 0.0, 1.0);
  output.uv = vec2<f32>(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
  return output;
}

@fragment
fn fs_composite(input: CompositeOutput) -> @location(0) vec4<f32> {
  return textureSample(offscreen_color, offscreen_sampler, input.uv);
}
