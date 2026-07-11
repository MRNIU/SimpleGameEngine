// Copyright The SimpleGameEngine Contributors

use ecs::Projection;
use math::{Mat4, Quat, Vec3, Vec4};

use crate::ViewportView;

pub const DEFAULT_NEAR_PLANE: f32 = 0.1;
pub const DEFAULT_FAR_PLANE: f32 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportSize {
    width: f32,
    height: f32,
}

impl ViewportSize {
    #[must_use]
    pub fn new(width: f32, height: f32) -> Option<Self> {
        (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
            .then_some(Self { width, height })
    }

    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }

    #[must_use]
    pub fn aspect_ratio(self) -> f32 {
        self.width / self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportClipPlanes {
    near: f32,
    far: f32,
}

impl ViewportClipPlanes {
    pub const DEFAULT: Self = Self {
        near: DEFAULT_NEAR_PLANE,
        far: DEFAULT_FAR_PLANE,
    };

    #[must_use]
    pub fn new(near: f32, far: f32) -> Option<Self> {
        (near.is_finite() && far.is_finite() && near > 0.0 && far > near)
            .then_some(Self { near, far })
    }

    #[must_use]
    pub const fn near(self) -> f32 {
        self.near
    }

    #[must_use]
    pub const fn far(self) -> f32 {
        self.far
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldRay {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportProjectionMatrix {
    view_projection: Mat4,
    inverse_view_projection: Mat4,
    camera_position: Vec3,
    perspective: bool,
}

impl ViewportProjectionMatrix {
    #[must_use]
    pub fn from_view(
        view: &ViewportView,
        size: ViewportSize,
        clip_planes: ViewportClipPlanes,
    ) -> Option<Self> {
        let camera_position = Vec3::from_array(view.transform.translation);
        let camera_rotation = normalized_quaternion(view.transform.rotation);
        if !camera_position.is_finite() || !camera_rotation.is_finite() {
            return None;
        }
        let view_matrix = Mat4::from_quat(camera_rotation.inverse())
            * Mat4::from_translation(-camera_position);
        let (projection_matrix, perspective) = match view.projection {
            Projection::Perspective { fov_y_degrees } => (
                perspective_matrix(fov_y_degrees, size, clip_planes)?,
                true,
            ),
            Projection::Orthographic { vertical_size } => (
                orthographic_matrix(vertical_size, size, clip_planes)?,
                false,
            ),
        };
        let view_projection = projection_matrix * view_matrix;
        let inverse_view_projection = view_projection.inverse();
        if !view_projection.is_finite() || !inverse_view_projection.is_finite() {
            return None;
        }
        Some(Self {
            view_projection,
            inverse_view_projection,
            camera_position,
            perspective,
        })
    }

    #[must_use]
    pub fn view_projection_array(self) -> [f32; 16] {
        self.view_projection.to_cols_array()
    }

    #[must_use]
    pub fn project_world_point(self, world: [f32; 3]) -> Option<[f32; 2]> {
        let world = Vec3::from_array(world);
        if !world.is_finite() {
            return None;
        }
        let clip = self.view_projection * world.extend(1.0);
        if !clip.is_finite() || clip.w <= f32::EPSILON {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        (ndc.is_finite() && (0.0..=1.0).contains(&ndc.z)).then_some([ndc.x, ndc.y])
    }

    #[must_use]
    pub fn project_world_segment(
        self,
        start: [f32; 3],
        end: [f32; 3],
    ) -> Option<[[f32; 2]; 2]> {
        let start = Vec3::from_array(start);
        let end = Vec3::from_array(end);
        if !start.is_finite() || !end.is_finite() {
            return None;
        }
        let start = self.view_projection * start.extend(1.0);
        let end = self.view_projection * end.extend(1.0);
        let (start, end) = clip_segment(start, end)?;
        if start.w <= f32::EPSILON || end.w <= f32::EPSILON {
            return None;
        }
        let start = start.truncate() / start.w;
        let end = end.truncate() / end.w;
        (start.is_finite() && end.is_finite()).then_some([[start.x, start.y], [end.x, end.y]])
    }

    #[must_use]
    pub fn screen_ray(self, ndc: [f32; 2]) -> Option<WorldRay> {
        if !ndc.into_iter().all(f32::is_finite) {
            return None;
        }
        let near = unproject(self.inverse_view_projection, [ndc[0], ndc[1], 0.0])?;
        let far = unproject(self.inverse_view_projection, [ndc[0], ndc[1], 1.0])?;
        let direction = (far - near).normalize_or_zero();
        if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
            return None;
        }
        Some(WorldRay {
            origin: if self.perspective {
                self.camera_position.to_array()
            } else {
                near.to_array()
            },
            direction: direction.to_array(),
        })
    }
}

fn perspective_matrix(
    fov_y_degrees: f32,
    size: ViewportSize,
    clip_planes: ViewportClipPlanes,
) -> Option<Mat4> {
    let fov_y = fov_y_degrees.clamp(1.0, 179.0).to_radians();
    let focal_y = 1.0 / (0.5 * fov_y).tan();
    let focal_x = focal_y / size.aspect_ratio();
    let depth = clip_planes.far / (clip_planes.far - clip_planes.near);
    let matrix = Mat4::from_cols_array(&[
        focal_x,
        0.0,
        0.0,
        0.0,
        0.0,
        focal_y,
        0.0,
        0.0,
        0.0,
        0.0,
        depth,
        1.0,
        0.0,
        0.0,
        -clip_planes.near * depth,
        0.0,
    ]);
    matrix.is_finite().then_some(matrix)
}

fn orthographic_matrix(
    vertical_size: f32,
    size: ViewportSize,
    clip_planes: ViewportClipPlanes,
) -> Option<Mat4> {
    if !vertical_size.is_finite() || vertical_size <= 0.0 {
        return None;
    }
    let half_height = vertical_size * 0.5;
    let half_width = half_height * size.aspect_ratio();
    let depth = 1.0 / (clip_planes.far - clip_planes.near);
    let matrix = Mat4::from_cols_array(&[
        1.0 / half_width,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / half_height,
        0.0,
        0.0,
        0.0,
        0.0,
        depth,
        0.0,
        0.0,
        0.0,
        -clip_planes.near * depth,
        1.0,
    ]);
    matrix.is_finite().then_some(matrix)
}

fn clip_segment(start: Vec4, end: Vec4) -> Option<(Vec4, Vec4)> {
    let planes: [fn(Vec4) -> f32; 6] = [
        |p| p.x + p.w,
        |p| p.w - p.x,
        |p| p.y + p.w,
        |p| p.w - p.y,
        |p| p.z,
        |p| p.w - p.z,
    ];
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for plane in planes {
        let at_start = plane(start);
        let at_end = plane(end);
        if at_start < 0.0 && at_end < 0.0 {
            return None;
        }
        if (at_start < 0.0) != (at_end < 0.0) {
            let crossing = at_start / (at_start - at_end);
            if at_start < 0.0 {
                lower = lower.max(crossing);
            } else {
                upper = upper.min(crossing);
            }
        }
    }
    (lower <= upper).then(|| {
        let delta = end - start;
        (start + delta * lower, start + delta * upper)
    })
}

fn unproject(inverse: Mat4, ndc: [f32; 3]) -> Option<Vec3> {
    let homogeneous = inverse * Vec3::from_array(ndc).extend(1.0);
    if !homogeneous.is_finite() || homogeneous.w.abs() <= f32::EPSILON {
        return None;
    }
    let world = homogeneous.truncate() / homogeneous.w;
    world.is_finite().then_some(world)
}

fn normalized_quaternion(rotation: [f32; 4]) -> Quat {
    let rotation = Quat::from_array(rotation);
    if !rotation.is_finite() || rotation.length_squared() <= f32::EPSILON {
        Quat::IDENTITY
    } else {
        rotation.normalize()
    }
}
