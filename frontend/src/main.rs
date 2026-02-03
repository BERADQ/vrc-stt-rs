use std::sync::mpsc;

use eframe::egui;
use rust_i18n::{i18n, t};
mod backend_manager;
mod constants;
mod gui;
mod socket;
mod utils;

i18n!("../locales", fallback = "en");

fn set_locale(lang: String) {
    rust_i18n::set_locale(&lang);
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let (msg_tx, msg_rx) = mpsc::channel();

    // Start Unix socket server BEFORE starting backend (backend will connect to us)
    let socket_path = std::env::var("VRC_STT_SOCKET")
        .unwrap_or_else(|_| common::default_socket_path().to_string());

    let mut socket_server = socket::SocketServer::new(socket_path, msg_tx.clone());
    match socket_server.start() {
        Ok(()) => {
            log::info!("{}", t!("socket.server.start.success"));
        }
        Err(e) => {
            log::error!("{}", t!("socket.server.start.failed", error = e));
            let _ = msg_tx.send(socket::FrontendMessage::BackendDisconnected);
        }
    }

    // Load configuration using ConfigManager
    let config_manager = common::ConfigManager::load().unwrap_or_else(|e| {
        log::error!("{}", t!("config.load.failed", error = e));
        // Fallback to default config
        let config = common::Config::default();
        let path = common::config::system_config_path();
        common::ConfigManager::new(config, path)
    });
    let config = config_manager.config();

    // Use the interface language from config if it's in our supported languages
    for supported_lang in common::ALL_LANG {
        if supported_lang[1] == config.interface_language.as_str() {
            rust_i18n::set_locale(&config.interface_language);
            break;
        }
    }

    // Create and start the backend manager AFTER socket is listening
    let mut backend_manager = backend_manager::BackendManager::new();

    // Attempt to start the backend process
    match backend_manager.start_backend() {
        Ok(()) => {
            log::info!("{}", t!("backend.start.success"));
        }
        Err(e) => {
            log::error!("{}", t!("backend.start.failed", error = e));
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([
                constants::DEFAULT_WINDOW_WIDTH,
                constants::DEFAULT_WINDOW_HEIGHT,
            ])
            .with_min_inner_size([constants::MIN_WINDOW_WIDTH, constants::MIN_WINDOW_HEIGHT]),
        ..Default::default()
    };

    eframe::run_native(
        "VRC STT",
        options,
        Box::new(|_cc| {
            Ok(Box::new(gui::VrcSttApp::new(
                msg_rx,
                backend_manager,
                socket_server,
            )))
        }),
    )
}
