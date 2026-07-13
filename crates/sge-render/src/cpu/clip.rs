// Copyright The SimpleGameEngine Contributors

use sge_math::{Vec3, Vec4};

#[derive(Debug, Clone, Copy)]
pub(super) struct ClipVertex {
    pub(super) position: Vec4,
    pub(super) normal: Vec3,
    pub(super) barycentric: Vec3,
}

impl ClipVertex {
    fn interpolate(self, other: Self, amount: f32) -> Self {
        Self {
            position: self.position.lerp(other.position, amount),
            normal: self.normal.lerp(other.normal, amount),
            barycentric: self.barycentric.lerp(other.barycentric, amount),
        }
    }
}

pub(super) fn clip_triangle(triangle: [ClipVertex; 3]) -> Vec<[ClipVertex; 3]> {
    let planes: [fn(Vec4) -> f32; 6] = [
        |position| position.x + position.w,
        |position| position.w - position.x,
        |position| position.y + position.w,
        |position| position.w - position.y,
        |position| position.z,
        |position| position.w - position.z,
    ];
    let mut polygon = triangle.to_vec();
    for distance in planes {
        polygon = clip_polygon(&polygon, distance);
        if polygon.len() < 3 {
            return Vec::new();
        }
    }
    (1..polygon.len() - 1)
        .map(|index| [polygon[0], polygon[index], polygon[index + 1]])
        .collect()
}

fn clip_polygon(vertices: &[ClipVertex], distance: fn(Vec4) -> f32) -> Vec<ClipVertex> {
    let mut output = Vec::new();
    let Some(mut previous) = vertices.last().copied() else {
        return output;
    };
    let mut previous_distance = distance(previous.position);
    for current in vertices.iter().copied() {
        let current_distance = distance(current.position);
        let previous_inside = previous_distance >= 0.0;
        let current_inside = current_distance >= 0.0;
        if previous_inside != current_inside {
            let amount = previous_distance / (previous_distance - current_distance);
            output.push(previous.interpolate(current, amount));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_distance = current_distance;
    }
    output
}
