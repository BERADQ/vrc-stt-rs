//! Utility functions for the frontend application

use chrono::{DateTime, Local};
use eframe::egui::{Color32, RichText};

/// Helper function to format timestamps consistently
pub fn format_timestamp(timestamp: DateTime<Local>) -> String {
    timestamp.format("%H:%M:%S").to_string()
}

/// Helper function to create colored text for UI
pub fn colored_text(text: &str, color: Color32, size: f32) -> RichText {
    RichText::new(text).color(color).size(size)
}
