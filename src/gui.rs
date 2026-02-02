use chrono;
use eframe::egui;
use std::sync::{mpsc, Arc, Once};

const FONT: &[u8] = include_bytes!("assets/font.ttf");

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "default_font".to_owned(),
        Arc::new(egui::FontData::from_static(FONT)),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "default_font".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "default_font".to_owned());

    ctx.set_fonts(fonts);
}

#[derive(Clone)]
pub struct HistoryItem {
    text: String,
    timestamp: chrono::DateTime<chrono::Local>,
}

pub struct VrcSttApp {
    text_rx: mpsc::Receiver<String>,
    current_text: String,
    font_once: Once,
    history: Vec<HistoryItem>,
    max_history: usize,
}

impl VrcSttApp {
    pub fn new(text_rx: mpsc::Receiver<String>) -> Self {
        Self {
            text_rx,
            current_text: String::new(),
            font_once: Once::new(),
            history: Vec::new(),
            max_history: 64,
        }
    }

    pub fn setup_fonts_once(&mut self, ctx: &egui::Context) {
        self.font_once.call_once(|| {
            setup_fonts(ctx);
        });
    }

    fn add_to_history(&mut self, text: String) {
        let item = HistoryItem {
            text: text.clone(),
            timestamp: chrono::Local::now(),
        };

        self.history.insert(0, item);

        if self.history.len() > self.max_history {
            self.history.truncate(self.max_history);
        }
    }

    fn format_timestamp(&self, timestamp: chrono::DateTime<chrono::Local>) -> String {
        timestamp.format("%H:%M:%S").to_string()
    }
}

impl eframe::App for VrcSttApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.setup_fonts_once(ctx);

        if let Ok(text) = self.text_rx.try_recv() {
            self.current_text = text.clone();
            self.add_to_history(text);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Current text display
            ui.horizontal(|ui| {
                ui.label("当前:");
                ui.label(
                    egui::RichText::new(&self.current_text)
                        .color(egui::Color32::from_rgb(100, 200, 100))
                        .size(16.0),
                );
            });

            ui.separator();

            // History section
            ui.heading("历史记录");

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    if self.history.is_empty() {
                        ui.label(
                            egui::RichText::new("暂无历史记录")
                                .color(egui::Color32::from_gray(128))
                                .italics(),
                        );
                    } else {
                        for (i, item) in self.history.iter().enumerate() {
                            ui.horizontal(|ui| {
                                // Timestamp
                                ui.label(
                                    egui::RichText::new(self.format_timestamp(item.timestamp))
                                        .color(egui::Color32::from_gray(150))
                                        .size(12.0),
                                );

                                // Index
                                ui.label(
                                    egui::RichText::new(format!("#{}", i + 1))
                                        .color(egui::Color32::from_gray(180))
                                        .size(12.0),
                                );

                                // Text
                                ui.label(
                                    egui::RichText::new(&item.text)
                                        .color(egui::Color32::from_rgb(200, 200, 200))
                                        .size(14.0),
                                );
                            });

                            if i < self.history.len() - 1 {
                                ui.add_space(4.0);
                            }
                        }
                    }
                });

            ui.separator();

            // Status bar
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "历史记录: {}/{}",
                        self.history.len(),
                        self.max_history
                    ))
                    .color(egui::Color32::from_gray(150))
                    .size(12.0),
                );
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
