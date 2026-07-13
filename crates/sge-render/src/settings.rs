// Copyright The SimpleGameEngine Contributors

//! Per-frame render debug settings that stay outside scene and asset data.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderMode {
    #[default]
    Lit,
    Unlit,
    Wireframe,
    LitWireframe,
}

impl RenderMode {
    pub const ALL: [Self; 4] = [Self::Lit, Self::Unlit, Self::Wireframe, Self::LitWireframe];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lit => "lit",
            Self::Unlit => "unlit",
            Self::Wireframe => "wireframe",
            Self::LitWireframe => "lit-wireframe",
        }
    }

    #[must_use]
    pub const fn has_fill(self) -> bool {
        matches!(self, Self::Lit | Self::Unlit | Self::LitWireframe)
    }

    #[must_use]
    pub const fn has_wireframe(self) -> bool {
        matches!(self, Self::Wireframe | Self::LitWireframe)
    }

    #[must_use]
    pub const fn is_lit(self) -> bool {
        matches!(self, Self::Lit | Self::LitWireframe)
    }

    pub(crate) const fn shader_code(self) -> f32 {
        match self {
            Self::Lit => 0.0,
            Self::Unlit => 1.0,
            Self::Wireframe => 2.0,
            Self::LitWireframe => 3.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSettings {
    mode: RenderMode,
    wireframe_width_pixels: u32,
}

impl RenderSettings {
    #[must_use]
    pub const fn new(mode: RenderMode, wireframe_width_pixels: u32) -> Self {
        Self {
            mode,
            wireframe_width_pixels: if wireframe_width_pixels == 0 {
                1
            } else {
                wireframe_width_pixels
            },
        }
    }

    #[must_use]
    pub const fn mode(self) -> RenderMode {
        self.mode
    }

    #[must_use]
    pub const fn wireframe_width_pixels(self) -> u32 {
        self.wireframe_width_pixels
    }
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self::new(RenderMode::Lit, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_player_safe_and_zero_width_is_normalized() {
        assert_eq!(RenderSettings::default().mode(), RenderMode::Lit);
        assert_eq!(
            RenderSettings::new(RenderMode::Wireframe, 0).wireframe_width_pixels(),
            1
        );
        assert_eq!(
            RenderMode::ALL.map(RenderMode::as_str),
            ["lit", "unlit", "wireframe", "lit-wireframe",]
        );
    }
}
