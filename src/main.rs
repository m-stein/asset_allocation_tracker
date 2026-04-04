mod app;
mod domain;
mod infrastructure;
mod ui;

use app::asset_service::AssetService;
use eframe::egui;
use infrastructure::sqlite_asset_repository::SqliteAssetRepository;
use ui::desktop_app::DesktopApp;

const DB_PATH: &str = "./data/assets.sdb";

fn main() -> eframe::Result<()> {
    let repository = SqliteAssetRepository::new(DB_PATH)
        .unwrap_or_else(|err| panic!("Failed to initialize SQLite repository: {err}"));

    let service = AssetService::new(Box::new(repository));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 220.0])
            .with_title("Asset Allocation Tracker"),
        ..Default::default()
    };

    eframe::run_native(
        "Asset Allocation Tracker",
        options,
        Box::new(move |_cc| Ok(Box::new(DesktopApp::new(service)))),
    )
}