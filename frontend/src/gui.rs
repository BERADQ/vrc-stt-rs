use chrono::{DateTime, Local};
use common::{config, Config, ConfigManager};
use eframe::egui::{self, Color32, Context, RichText, Ui};
use rust_i18n::t;
use std::sync::{mpsc, Arc};

use crate::backend_manager::BackendManager;
use crate::constants;
use crate::socket::{FrontendMessage, SocketServer};
use crate::utils;

/// Recording status states
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RecordingStatus {
    Idle,
    Recording,
    Processing,
    Error,
}

impl RecordingStatus {
    fn label(self) -> String {
        match self {
            Self::Idle => t!("status.idle").to_string(),
            Self::Recording => t!("status.recording").to_string(),
            Self::Processing => t!("status.processing").to_string(),
            Self::Error => t!("status.error").to_string(),
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Idle => Color32::from_gray(128),
            Self::Recording => Color32::from_rgb(100, 255, 100),
            Self::Processing => Color32::from_rgb(255, 200, 100),
            Self::Error => Color32::from_rgb(255, 50, 50),
        }
    }
}

/// View state for panel switching
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewState {
    Main,
    Settings,
    Logs,
    About,
}

/// Single history entry
#[derive(Clone)]
pub struct HistoryItem {
    text: String,
    timestamp: DateTime<Local>,
    is_error: bool,
}

impl HistoryItem {
    fn new(text: String, is_error: bool) -> Self {
        Self {
            text,
            timestamp: Local::now(),
            is_error,
        }
    }

    pub fn timestamp_str(&self) -> String {
        utils::format_timestamp(self.timestamp)
    }
}

use crate::backend_manager::BackendLogMessage;

/// Log item for display
#[derive(Clone)]
pub struct LogItem {
    message: String,
    timestamp: DateTime<Local>,
    log_type: LogType,
}

#[derive(Clone)]
pub enum LogType {
    InternalInfo,
    InternalError,
    BackendOutput,
    BackendLog,
}

impl LogItem {
    fn new(message: String, log_type: LogType) -> Self {
        Self {
            message,
            timestamp: Local::now(),
            log_type,
        }
    }

    pub fn timestamp_str(&self) -> String {
        utils::format_timestamp(self.timestamp)
    }

    pub fn color(&self) -> Color32 {
        match self.log_type {
            LogType::InternalInfo => Color32::from_rgb(100, 150, 255), // Blue
            LogType::InternalError => Color32::from_rgb(255, 100, 100), // Red
            LogType::BackendOutput => Color32::from_rgb(200, 200, 200), // Gray
            LogType::BackendLog => Color32::from_rgb(180, 180, 180), // Light Gray for backend logs from stderr
        }
    }
}

/// Main application state
pub struct VrcSttApp {
    msg_rx: mpsc::Receiver<FrontendMessage>,
    status: RecordingStatus,
    pub history: Vec<HistoryItem>,
    font_initialized: bool,
    view_state: ViewState,
    pub config_manager: ConfigManager,
    pub backend_manager: BackendManager,
    pub logs: Vec<LogItem>,
    max_log_lines: usize,

    // Socket server that listens for backend connections
    socket_server: Option<SocketServer>,

    // Performance optimization variables
    message_batch: Vec<FrontendMessage>,
    should_repaint: bool,
}

impl VrcSttApp {
    pub fn new(
        msg_rx: mpsc::Receiver<FrontendMessage>,
        backend_manager: BackendManager,
        socket_server: SocketServer,
    ) -> Self {
        let config_manager = ConfigManager::load().unwrap_or_else(|_| {
            // Fallback to default config if loading fails
            let config = Config::default();
            let path = config::system_config_path();
            ConfigManager::new(config, path)
        });

        Self {
            msg_rx,
            status: RecordingStatus::Idle,
            history: Vec::with_capacity(constants::MAX_HISTORY),
            font_initialized: false,
            view_state: ViewState::Main,
            config_manager,
            backend_manager,
            logs: Vec::new(),
            max_log_lines: constants::MAX_LOG_LINES,

            // Socket server
            socket_server: Some(socket_server),

            // Performance optimization variables
            message_batch: Vec::with_capacity(constants::MESSAGE_BATCH_CAPACITY),
            should_repaint: true,
        }
    }

    /// Restart the backend process
    pub fn restart_backend(&mut self) {
        if let Err(e) = self.backend_manager.restart_backend() {
            log::error!("{}", t!("backend.restart.failed", error = e));
            self.add_log(LogItem::new(
                t!("backend.restart.failed", error = e).to_string(),
                LogType::InternalError,
            ));
        } else {
            self.add_log(LogItem::new(
                t!("backend.connected.log").to_string(),
                LogType::InternalInfo,
            ));
        }
    }

    fn initialize_fonts(&mut self, ctx: &Context) {
        if !self.font_initialized {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "default_font".to_owned(),
                Arc::new(egui::FontData::from_static(constants::FONT_PATH)),
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
            self.font_initialized = true;
        }
    }

    fn process_messages(&mut self) {
        // Process backend manager logs first (no borrow conflicts)
        let backend_logs = self.backend_manager.process_logs();
        if !backend_logs.is_empty() {
            for log_msg in backend_logs {
                match log_msg {
                    BackendLogMessage::InternalInfo(msg) => {
                        self.add_log(LogItem::new(msg, LogType::InternalInfo));
                    }
                    BackendLogMessage::InternalError(msg) => {
                        self.add_log(LogItem::new(msg, LogType::InternalError));
                    }
                    BackendLogMessage::BackendOutput(msg) => {
                        self.add_log(LogItem::new(msg, LogType::BackendOutput));
                    }
                    BackendLogMessage::BackendLog(msg) => {
                        self.add_log(LogItem::new(msg, LogType::BackendLog));
                    }
                }
            }
            self.should_repaint = true;
        }

        // Batch process socket messages to reduce GUI updates
        self.message_batch.clear();

        while let Ok(msg) = self.msg_rx.try_recv() {
            self.message_batch.push(msg);
        }

        // Process all messages from batch to avoid borrowing issues
        let mut status_updates = Vec::new();
        let mut history_additions = Vec::new();
        let mut log_additions = Vec::new();

        for msg in self.message_batch.drain(..) {
            match msg {
                FrontendMessage::BackendConnected => {
                    // Reset to Idle when backend connects
                    status_updates.push(RecordingStatus::Idle);
                    log_additions.push(LogItem::new(
                        "Backend connected".to_string(),
                        LogType::InternalInfo,
                    ));
                    self.should_repaint = true;
                }
                FrontendMessage::BackendDisconnected => {
                    status_updates.push(RecordingStatus::Error);
                    self.should_repaint = true;
                    // Backend will auto-reconnect when it restarts
                }
                FrontendMessage::STTRecordStart => {
                    status_updates.push(RecordingStatus::Recording);
                    self.should_repaint = true;
                }
                FrontendMessage::STTRecordProcessing => {
                    status_updates.push(RecordingStatus::Processing);
                    self.should_repaint = true;
                }
                FrontendMessage::STTRecordEndWithText(text) => {
                    status_updates.push(RecordingStatus::Idle);
                    history_additions.push((text, false));
                    self.should_repaint = true;
                }
                FrontendMessage::STTRecordEndWithError(error) => {
                    status_updates.push(RecordingStatus::Idle);
                    history_additions.push((error, true));
                    self.should_repaint = true;
                }
            }
        }

        // Add all collected logs after the loop
        for log_item in log_additions {
            self.add_log(log_item);
        }

        // Apply all status updates
        for new_status in status_updates {
            self.status = new_status;
        }

        // Add all history items
        for (text, is_error) in history_additions {
            self.add_history(text, is_error);
        }
    }

    fn add_history(&mut self, text: String, is_error: bool) {
        self.history.insert(0, HistoryItem::new(text, is_error));
        if self.history.len() > constants::MAX_HISTORY {
            self.history.truncate(constants::MAX_HISTORY);
        }
    }

    fn add_log(&mut self, log_item: LogItem) {
        self.logs.push(log_item);
        // Keep only the last max_log_lines logs to prevent memory issues
        if self.logs.len() > self.max_log_lines {
            self.logs.drain(0..(self.logs.len() - self.max_log_lines));
        }
    }
}

impl eframe::App for VrcSttApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.initialize_fonts(ctx);
        self.process_messages();

        // Top panel with status and navigation
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(3.0); // Top padding
            ui.horizontal(|ui| {
                // Status on the left
                ui.label(
                    RichText::new(self.status.label())
                        .color(self.status.color())
                        .size(14.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Show different navigation based on current view
                    match self.view_state {
                        ViewState::Main => {
                            // On main view, show settings button
                            if ui.button(t!("navigation.settings")).clicked() {
                                self.view_state = ViewState::Settings;
                            }

                            // Button to go to logs page
                            if ui.button(t!("logs.tab.label")).clicked() {
                                self.view_state = ViewState::Logs;
                            }

                            // Button to go to about page
                            if ui.button(t!("navigation.about")).clicked() {
                                self.view_state = ViewState::About;
                            }
                        }
                        ViewState::Settings => {
                            // On settings view, show back to main, logs and about button
                            if ui.button(t!("logs.tab.label")).clicked() {
                                self.view_state = ViewState::Logs;
                            }

                            if ui.button(t!("navigation.about")).clicked() {
                                self.view_state = ViewState::About;
                            }

                            if ui
                                .button(RichText::new(t!("navigation.back")).strong())
                                .clicked()
                            {
                                self.view_state = ViewState::Main;
                            }
                        }
                        ViewState::Logs => {
                            // On logs view, show back to main, settings and about button
                            if ui.button(t!("navigation.settings")).clicked() {
                                self.view_state = ViewState::Settings;
                            }

                            if ui.button(t!("navigation.about")).clicked() {
                                self.view_state = ViewState::About;
                            }

                            if ui
                                .button(RichText::new(t!("navigation.back")).strong())
                                .clicked()
                            {
                                self.view_state = ViewState::Main;
                            }
                        }
                        ViewState::About => {
                            // On about view, show back to main, settings and logs button
                            if ui.button(t!("navigation.settings")).clicked() {
                                self.view_state = ViewState::Settings;
                            }

                            if ui.button(t!("logs.tab.label")).clicked() {
                                self.view_state = ViewState::Logs;
                            }

                            if ui
                                .button(RichText::new(t!("navigation.back")).strong())
                                .clicked()
                            {
                                self.view_state = ViewState::Main;
                            }
                        }
                    }
                });
            });
            ui.add_space(1.0); // Bottom padding
        });

        // Bottom panel: Status bar
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(3.0); // Top padding
            ui.horizontal(|ui| {
                // Left side: Status info
                ui.label(utils::colored_text(
                    &t!(
                        "history.count",
                        count = self.history.len(),
                        max = constants::MAX_HISTORY
                    ),
                    Color32::from_gray(150),
                    12.0,
                ));

                // Add restart backend button if disconnected
                if self.status == RecordingStatus::Error {
                    ui.separator();
                    if ui.button(t!("restart.backend.button")).clicked() {
                        self.restart_backend();
                    }
                }

                // Right side: Save/Reset buttons (only in settings view)
                if self.view_state == ViewState::Settings {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t!("settings.reset")).clicked() {
                            if let Err(e) = self.config_manager.reload() {
                                eprintln!("Failed to reload config: {}", e);
                            }

                            // Restart backend to apply reset configuration
                            if let Err(e) = self.backend_manager.restart_backend() {
                                log::error!(
                                    "{}",
                                    t!("config.reset.restart.backend.failed", error = e)
                                );
                            }
                        }

                        if ui.button(t!("settings.save")).clicked() {
                            let config = self.config_manager.config().clone();
                            if let Err(e) = self.config_manager.save() {
                                eprintln!("Failed to save config: {}", e);
                            }

                            // Restart backend to apply new configuration
                            if let Err(e) = self.backend_manager.restart_backend() {
                                log::error!(
                                    "{}",
                                    t!("config.save.restart.backend.failed", error = e)
                                );
                            }

                            // Backend will automatically connect to the frontend socket
                            // Apply language setting
                            if rust_i18n::locale().to_string() != config.interface_language {
                                crate::set_locale(config.interface_language.clone());
                            }
                        }
                    });
                }
            });
            ui.add_space(1.0); // Bottom padding
        });

        // Central panel: Content based on view state
        egui::CentralPanel::default().show(ctx, |ui| match self.view_state {
            ViewState::Main => self.render_history(ui),
            ViewState::Settings => self.render_settings(ui),
            ViewState::Logs => self.render_logs(ui),
            ViewState::About => self.render_about(ui),
        });

        // Only request repaint when necessary, based on actual changes
        if self.should_repaint {
            self.should_repaint = false;
            // Use a reasonable frame rate limit
            ctx.request_repaint_after(std::time::Duration::from_millis(
                constants::UI_UPDATE_INTERVAL_MS,
            ));
        } else {
            // Still request repaint occasionally to keep the UI responsive
            ctx.request_repaint_after(std::time::Duration::from_millis(
                constants::IDLE_REPAINT_INTERVAL_MS,
            ));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        log::info!("{}", t!("app.shutdown.start"));

        // Stop socket connection first
        if let Some(server) = self.socket_server.as_mut() {
            server.stop();
        }

        // Then stop the backend
        self.backend_manager.stop_backend();

        log::info!("{}", t!("app.shutdown.complete"));
    }
}

impl VrcSttApp {
    fn render_history(&self, ui: &mut Ui) {
        ui.heading(t!("history.records"));
        ui.add_space(8.0);

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if self.history.is_empty() {
                    ui.label(
                        utils::colored_text(
                            &t!("no.history.records"),
                            Color32::from_gray(128),
                            14.0,
                        )
                        .italics(),
                    );
                    return;
                }

                egui::Grid::new("history_grid")
                    .num_columns(3)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for (i, item) in self.history.iter().enumerate() {
                            // Timestamp
                            ui.label(utils::colored_text(
                                &item.timestamp_str(),
                                Color32::from_gray(150),
                                12.0,
                            ));

                            // Index
                            ui.label(utils::colored_text(
                                &format!("#{}", i + 1),
                                Color32::from_gray(180),
                                12.0,
                            ));

                            // Text
                            let text_color = if item.is_error {
                                Color32::from_rgb(255, 100, 100)
                            } else {
                                Color32::from_rgb(200, 200, 200)
                            };

                            ui.add(
                                egui::Label::new(utils::colored_text(&item.text, text_color, 14.0))
                                    .wrap(),
                            );

                            ui.end_row();
                        }
                    });
            });
    }

    fn render_settings(&mut self, ui: &mut Ui) {
        ui.heading(t!("settings.title"));
        ui.add_space(12.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Model settings
                ui.label(
                    RichText::new(t!("settings.model"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                let config = &mut self.config_manager.config_mut();
                ui.horizontal(|ui| {
                    ui.label(t!("settings.model.path"));
                    ui.add(egui::TextEdit::singleline(&mut config.model_path).desired_width(280.0));
                });

                ui.horizontal(|ui| {
                    ui.label(t!("settings.language"));
                    ui.add(egui::TextEdit::singleline(&mut config.language).desired_width(100.0));
                });

                let mut initial_prompt_str = config.initial_prompt.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(t!("settings.initial.prompt"));
                    ui.add(
                        egui::TextEdit::singleline(&mut initial_prompt_str).desired_width(280.0),
                    );
                });
                config.initial_prompt = if initial_prompt_str.is_empty() {
                    None
                } else {
                    Some(initial_prompt_str)
                };

                ui.add_space(20.0);

                // Interface language settings
                ui.label(
                    RichText::new(t!("settings.interface.lang"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                let config = &mut self.config_manager.config_mut();
                ui.label(t!("settings.interface.lang.note"));
                ui.add_space(4.0);

                egui::ComboBox::from_label("")
                    .selected_text(&config.interface_language)
                    .show_ui(ui, |ui| {
                        for lang in common::ALL_LANG {
                            ui.selectable_value(
                                &mut config.interface_language,
                                lang[1].to_string(),
                                lang[0],
                            );
                        }
                    });

                ui.add_space(20.0);

                // VAD settings
                ui.label(
                    RichText::new(t!("settings.vad"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                let config = &mut self.config_manager.config_mut();
                ui.horizontal(|ui| {
                    ui.label(t!("settings.vad.threshold"));
                    ui.add(
                        egui::Slider::new(&mut config.vad.threshold_level, 0.0..=1.0)
                            .show_value(true),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(t!("settings.vad.debounce"));
                    let mut debounce = config.vad.debounce_times as i32;
                    ui.add(egui::Slider::new(&mut debounce, 10..=500));
                    config.vad.debounce_times = debounce as usize;
                });

                ui.add_space(20.0);

                // Network settings
                ui.label(
                    RichText::new(t!("settings.network"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                let config = &mut self.config_manager.config_mut();
                ui.horizontal(|ui| {
                    ui.label(t!("settings.udp.port"));
                    let mut port = config.udp.port as i32;
                    ui.add(egui::DragValue::new(&mut port).speed(1).range(1..=65535));
                    config.udp.port = port as u16;
                });

                ui.horizontal(|ui| {
                    ui.label(t!("settings.udp.target"));
                    ui.add(egui::TextEdit::singleline(&mut config.udp.to).desired_width(200.0));
                });

                ui.add_space(20.0);

                // Audio settings
                ui.label(
                    RichText::new(t!("settings.audio"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                let config = &mut self.config_manager.config_mut();
                ui.horizontal(|ui| {
                    ui.label(t!("settings.audio.channel_mix_mode"));
                    egui::ComboBox::from_id_salt("channel_mix_mode")
                        .selected_text(match config.audio.channel_mix_mode {
                            common::ChannelMixMode::MixToMono => {
                                t!("settings.audio.mix_to_mono")
                            }
                            common::ChannelMixMode::FirstChannel => {
                                t!("settings.audio.first_channel")
                            }
                            common::ChannelMixMode::SecondChannel => {
                                t!("settings.audio.second_channel")
                            }
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut config.audio.channel_mix_mode,
                                common::ChannelMixMode::MixToMono,
                                t!("settings.audio.mix_to_mono"),
                            );
                            ui.selectable_value(
                                &mut config.audio.channel_mix_mode,
                                common::ChannelMixMode::FirstChannel,
                                t!("settings.audio.first_channel"),
                            );
                            ui.selectable_value(
                                &mut config.audio.channel_mix_mode,
                                common::ChannelMixMode::SecondChannel,
                                t!("settings.audio.second_channel"),
                            );
                        });
                });
            });
    }

    fn render_logs(&mut self, ui: &mut Ui) {
        ui.heading(t!("backend.logs.label"));
        ui.add_space(8.0);

        // Add controls for clearing logs
        ui.horizontal(|ui| {
            if ui.button(t!("clear.logs.button")).clicked() {
                self.logs.clear();
            }

            ui.label(t!("log.entries.count", count = self.logs.len()));
        });

        ui.add_space(8.0);

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .stick_to_bottom(true) // Auto-scroll to bottom
            .show(ui, |ui| {
                if self.logs.is_empty() {
                    ui.label(
                        utils::colored_text(&t!("no.backend.logs"), Color32::from_gray(128), 14.0)
                            .italics(),
                    );
                    return;
                }

                egui::Grid::new("logs_grid")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for item in &self.logs {
                            // Timestamp
                            ui.label(utils::colored_text(
                                &item.timestamp_str(),
                                Color32::from_gray(150),
                                11.0,
                            ));

                            // Log message
                            let text_color = item.color();
                            ui.add(
                                egui::Label::new(utils::colored_text(
                                    &item.message,
                                    text_color,
                                    12.0,
                                ))
                                .wrap(),
                            );

                            ui.end_row();
                        }
                    });
            });
    }

    fn render_about(&self, ui: &mut Ui) {
        ui.heading(t!("about.title"));
        ui.add_space(12.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Software information section
                ui.label(
                    RichText::new(t!("about.software.info"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                egui::Grid::new("software_info_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(utils::colored_text(
                            &t!("about.app.name"),
                            Color32::from_gray(180),
                            13.0,
                        ));
                        ui.label(env!("CARGO_PKG_NAME"));
                        ui.end_row();

                        ui.label(utils::colored_text(
                            &t!("about.version"),
                            Color32::from_gray(180),
                            13.0,
                        ));
                        ui.label(env!("CARGO_PKG_VERSION"));
                        ui.end_row();

                        ui.label(utils::colored_text(
                            &t!("about.repository"),
                            Color32::from_gray(180),
                            13.0,
                        ));
                        ui.hyperlink(env!("CARGO_PKG_REPOSITORY"));
                        ui.end_row();
                    });

                ui.add_space(20.0);

                // Libraries section
                ui.label(
                    RichText::new(t!("about.libraries.attribution"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                egui::Grid::new("libraries_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        let libraries = [
                            ("egui/eframe", "MIT/Apache-2.0"),
                            ("whisper.cpp", "MIT"),
                            ("cpal", "Apache-2.0"),
                            ("webrtc-vad", "MIT"),
                            ("rubato", "MIT"),
                            ("rosc", "MIT/Apache-2.0"),
                            ("chrono", "MIT/Apache-2.0"),
                            ("serde", "MIT/Apache-2.0"),
                            ("anyhow", "MIT/Apache-2.0"),
                            ("rust-i18n", "MIT"),
                            ("uds_windows", "MIT"),
                        ];

                        for (name, license) in libraries.iter() {
                            ui.label(*name);
                            ui.label(
                                RichText::new(format!("{}", license))
                                    .color(Color32::from_gray(140))
                                    .size(12.0),
                            );
                            ui.end_row();
                        }
                    });

                ui.add_space(20.0);

                // Licenses section
                ui.label(
                    RichText::new(t!("about.third.party.licenses"))
                        .color(Color32::from_rgb(100, 200, 255))
                        .size(16.0)
                        .strong(),
                );
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("MIT License").clicked() {
                        ui.ctx()
                            .copy_text(include_str!("../licenses/MIT.txt").to_string());
                    }
                    ui.label(
                        RichText::new("(📋 click to copy)")
                            .color(Color32::from_gray(140))
                            .size(11.0),
                    );
                });

                ui.horizontal(|ui| {
                    if ui.button("Apache-2.0 License").clicked() {
                        ui.ctx()
                            .copy_text(include_str!("../licenses/APACHE-2.0.txt").to_string());
                    }
                    ui.label(
                        RichText::new("(📋 click to copy)")
                            .color(Color32::from_gray(140))
                            .size(11.0),
                    );
                });

                // CUDA redistribution notice - only on Windows with cuda feature enabled
                #[cfg(all(windows, feature = "cuda"))]
                {
                    ui.add_space(20.0);

                    ui.label(
                        RichText::new(t!("about.cuda.redistribution"))
                            .color(Color32::from_rgb(100, 200, 255))
                            .size(16.0)
                            .strong(),
                    );
                    ui.separator();
                    ui.add_space(4.0);

                    ui.hyperlink_to(
                        "📄 CUDA Toolkit End User License Agreement",
                        "https://docs.nvidia.com/cuda/eula/index.html",
                    );
                }
            });
    }
}
