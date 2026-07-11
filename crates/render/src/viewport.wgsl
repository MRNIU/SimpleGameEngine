// Copyright The SimpleGameEngine Contributors
//
// wgpu viewport mesh and composite shaders.

struct CameraUniform {
  view_projection: mat4x4<f32>,
  grid_u_step: vec4<f32>,
  grid_v_radius: vec4<f32>,
  grid_camera_radius: vec4<f32>,
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

struct GridOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) world_position: vec3<f32>,
};

@vertex
fn vs_grid_plane(input: VertexInput) -> GridOutput {
  var output: GridOutput;
  output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
  output.world_position = input.position;
  return output;
}

fn axis_color(axis: vec3<f32>) -> vec3<f32> {
  if abs(axis.x) > 0.5 {
    return vec3<f32>(0.82, 0.20, 0.24);
  }
  if abs(axis.y) > 0.5 {
    return vec3<f32>(0.18, 0.68, 0.32);
  }
  return vec3<f32>(0.20, 0.45, 0.90);
}

fn grid_coverage(coordinate: vec2<f32>) -> f32 {
  let width = max(fwidth(coordinate), vec2<f32>(0.0001));
  let distance_to_line = abs(fract(coordinate - 0.5) - 0.5) / width;
  let line = 1.0 - min(min(distance_to_line.x, distance_to_line.y), 1.0);
  return line * min(1.0, 1.0 / max(width.x, width.y));
}

@fragment
fn fs_grid_plane(input: GridOutput) -> @location(0) vec4<f32> {
  let axis_u = camera.grid_u_step.xyz;
  let axis_v = camera.grid_v_radius.xyz;
  let step = max(camera.grid_u_step.w, 0.0001);
  let radius = max(camera.grid_v_radius.w, step);
  let coordinate = vec2<f32>(
    dot(input.world_position, axis_u),
    dot(input.world_position, axis_v),
  );

  let minor = grid_coverage(coordinate / step);
  let major = grid_coverage(coordinate / (step * 10.0));
  var color = vec3<f32>(0.24, 0.28, 0.31);
  var alpha = max(minor * 0.24, major * 0.48);

  let world_width = max(fwidth(coordinate), vec2<f32>(0.0001));
  if abs(coordinate.y) <= world_width.y * 1.25 {
    color = axis_color(axis_u);
    alpha = 0.82;
  }
  if abs(coordinate.x) <= world_width.x * 1.25 {
    color = axis_color(axis_v);
    alpha = 0.82;
  }

  let normal = normalize(cross(axis_u, axis_v));
  let camera_position = camera.grid_camera_radius.xyz;
  let plane_center = camera_position - normal * dot(camera_position, normal);
  let distance_from_center = length(input.world_position - plane_center);
  let distance_fade = 1.0 - smoothstep(radius * 0.72, radius, distance_from_center);
  let view_direction = normalize(camera_position - input.world_position);
  let angle_fade = smoothstep(0.02, 0.16, abs(dot(view_direction, normal)));
  alpha *= distance_fade * angle_fade;

  if alpha < 0.01 {
    discard;
  }
  return vec4<f32>(color, alpha);
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
