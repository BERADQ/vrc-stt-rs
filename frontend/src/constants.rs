//! Constants used throughout the frontend application

pub const FONT_PATH: &[u8] = include_bytes!("assets/font.ttf");
pub const MAX_HISTORY: usize = 64;
pub const MAX_LOG_LINES: usize = 500;
pub const DEFAULT_WINDOW_WIDTH: f32 = 400.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 600.0;
pub const MIN_WINDOW_WIDTH: f32 = 300.0;
pub const MIN_WINDOW_HEIGHT: f32 = 400.0;
pub const UI_UPDATE_INTERVAL_MS: u64 = 16; // ~60 FPS
pub const IDLE_REPAINT_INTERVAL_MS: u64 = 100; // Less frequent when no changes
pub const MESSAGE_BATCH_CAPACITY: usize = 16;
