// Copyright The SimpleGameEngine Contributors
//
//! winit 窗口配置边界。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl WindowConfig {
    #[must_use]
    pub fn new(title: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            title: title.into(),
            width,
            height,
        }
    }

    #[must_use]
    pub fn logical_size(&self) -> winit::dpi::LogicalSize<u32> {
        winit::dpi::LogicalSize::new(self.width, self.height)
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self::new("SimpleGameEngine", 1280, 720)
    }
}

#[cfg(test)]
mod tests {
    use super::WindowConfig;

    #[test]
    fn keeps_window_size_in_config() {
        let config = WindowConfig::new("Editor", 800, 600);
        let size = config.logical_size();

        assert_eq!(size.width, 800);
        assert_eq!(size.height, 600);
    }
}
