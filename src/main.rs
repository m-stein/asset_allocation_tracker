mod app;
mod domain;
mod infra;
mod ui;

use app::asset_service::AssetService;
use eframe::egui;
use infra::sqlite_asset_repository::SqliteAssetRepository;
use ui::desktop_app::desktop_app::DesktopApp;

const DB_PATH: &str = "./data/allocation_records.sdb";

fn main() -> eframe::Result<()> {
    let repository = SqliteAssetRepository::new(DB_PATH)
        .unwrap_or_else(|err| panic!("Failed to initialize SQLite repository: {err}"));

    let service = AssetService::new(Box::new(repository));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_title("Asset Allocation Tracker"),
        ..Default::default()
    };

    eframe::run_native(
        "Asset Allocation Tracker",
        options,
        Box::new(move |_cc| Ok(Box::new(DesktopApp::new(service)))),
    )
}