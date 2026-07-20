use crate::app_backend::AppBackend;
use crate::percent_stacked_bar_chart::draw_percent_stacked_bar_chart;
use crate::png::load_png_texture_from_bytes;
use core_lib::{
    APP_NAME, AddAssetArgs, AllocationDiagramData, AllocationPositionInput, AllocationRecord,
    AssetReferenceType, Category, CategoryAssignmentPc, CategoryValueInput,
    ConfigureCatgoriesInput, GetAllocDiagramDataArgs, ListedTransaction, LogBuyTransactionInput,
    LogSellTransactionInput, NewCategoryInput, PortfolioIsinItem, PortfolioOverviewItem,
    TransactionAsset, TransactionType, with_gui_requests,
};
use eframe::egui;
use egui::{TextEdit, TextWrapMode, Widget};
use egui_extras::DatePickerButton;
use jiff::{Zoned, civil::Date};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use strum::IntoEnumIterator;

macro_rules! define_request_data {
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        paste::paste! {
            #[derive(Default)]
            #[allow(dead_code)]
            struct RequestData {
                $([<$request _rx>]: Option<Receiver<eyre::Result<$ret_ty>>>,)*
            }
        }
    }
}

with_gui_requests!(define_request_data);

macro_rules! implement_requests {

    // For each request, redirect to one of the @func arms depending on whether
    // the request has an argument or not
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        $(
            implement_requests!(@maybe_request $access $request ($($arg_ty)?) -> $ret_ty);
        )*
    };
    (@maybe_request Public $request:ident ($($arg_ty:ty)?) -> $ret_ty:ty) => {};
    (@maybe_request Token $request:ident ($($arg_ty:ty)?) -> $ret_ty:ty) => {
        implement_requests!(@start_req_fn $request ($($arg_ty)?) -> $ret_ty);
        paste::paste! {
            fn [<poll_ $request _rx>](&mut self) -> Option<$ret_ty> {
                let mut res: Option<$ret_ty> = None;
                if let Some(rx) = &self.request_data.[<$request _rx>]
                    && let Ok(result) = rx.try_recv()
                {
                    match result {
                        Ok(result) => {
                            self.message = None;
                            res = Some(result);
                        }
                        Err(error) => {
                            self.message = Some(error.to_string());
                        }
                    }
                    self.request_data.[<$request _rx>] = None;
                    self.decr_pending_req_cnt();
                }
                res
            }
        }
    };
    // Start-request function-template for requests without an argument
    (@start_req_fn $request:ident () -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&mut self) {
                self.message = None;
                self.request_data.[<$request _rx>] = Some(self.backend.[<start_ $request>]());
                self.incr_pending_req_cnt();
            }
        }
    };
    // Start-request function-template for requests with one argument
    (@start_req_fn $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&mut self, arg: $arg_ty) {
                self.message = None;
                self.request_data.[<$request _rx>] = Some(self.backend.[<start_ $request>](arg));
                self.incr_pending_req_cnt();
            }
        }
    };
}

fn trunc_str(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let truncated: String = value.chars().take(max_chars).collect();
    format!("{truncated}...")
}

static UNKNOWN_STR: &str = "-";

fn value_or_unknown(value: Option<&str>) -> &str {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(UNKNOWN_STR)
}

fn format_updated_at(date: Option<&str>, time: Option<&str>) -> String {
    let date = date.filter(|date| !date.trim().is_empty());
    let time = time
        .filter(|time| !time.trim().is_empty())
        .map(|time| time.chars().take(5).collect::<String>());

    match (date, time) {
        (Some(date), Some(time)) => format!("{date} {time}"),
        (Some(date), None) => date.to_string(),
        (None, Some(time)) => time,
        (None, None) => "?".to_string(),
    }
}

#[derive(PartialEq)]
enum Page {
    MainMenu,
    AllocationDiagram,
    AddAsset,
    ConfigureCategories,
    AddAllocationRecord,
    LogBuyTransaction,
    Transactions,
    LogSellTransaction,
    TransactionAssets,
    ImportTransactionAssets,
}

pub struct PositionItem {
    pub asset_id: i64,
    pub label: String,
    pub amount: String,
}

#[allow(unused)]
pub struct EframeApp<B: AppBackend> {
    backend: B,
    message: Option<String>,
    pending_req_cnt: usize,
    asset_name_by_id: HashMap<i64, String>,
    allocation_record_date: Date,
    allocation_record_assets: Vec<PositionItem>,
    alloc_diagram_category_id: Option<i64>,
    alloc_diagram_data: Option<AllocationDiagramData>,
    latest_record: Option<AllocationRecord>,
    categories: Vec<Category>,
    add_asset_args: AddAssetArgs,
    log_buy_transaction_input: LogBuyTransactionInput,
    listed_transactions: Vec<ListedTransaction>,
    transaction_asset_isins_input: String,
    transaction_assets: Vec<TransactionAsset>,
    portfolio_overview_items: Vec<PortfolioOverviewItem>,
    portfolio_isin_items: Vec<PortfolioIsinItem>,
    portfolio_isin: Option<String>,
    portfolio_asset_name: Option<String>,
    log_sell_transaction_input: LogSellTransactionInput,
    cfg_catgs_input: ConfigureCatgoriesInput,
    request_data: RequestData,
    page: Page,
    squirrel_texture: Option<egui::TextureHandle>,
}

impl<BACKEND: AppBackend> EframeApp<BACKEND> {
    const H1_SIZE: f32 = 32.0;
    const H2_SIZE: f32 = 24.0;
    const H3_SIZE: f32 = 18.0;
    const SPACE_1: f32 = 8.0;
    const SPACE_2: f32 = 12.0;
    const SPACE_3: f32 = 24.0;
    const DEFAULT_INPUT_HEIGHT: f32 = 19.0;
    const DEFAULT_INPUT_WIDTH: f32 = 150.0;
    const HELP_POPUP_WIDTH: f32 = 260.0;
    const DECIMAL_DISPLAY_MAX_FRACTION_DIGITS: usize = 10;
    const SYM_BTN_SIZE: f32 = Self::DEFAULT_INPUT_HEIGHT;
    const BACK_BTN_SIZE: f32 = 24.0;
    const BACK_BTN_FONT_SIZE: f32 = 18.0;
    const BACK_BTN_RMARGIN: f32 = 10.0;
    const TABLE_GRID_SPACING: [f32; 2] = [16.0, 16.0];
    const TRANSACTION_ASSETS_GRID_SPACING: [f32; 2] = [20.0, 20.0];
    const TRANSACTION_ASSET_ROW_LINE_SPACING: f32 = 4.0;
    const TRANSACTION_ACTION_BTN_SIZE: [f32; 2] = [80.0, 32.0];
    const BUY_QUANTITY_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 190, 95);
    const SELL_QUANTITY_COLOR: egui::Color32 = egui::Color32::from_rgb(235, 80, 95);
    const CLICKABLE_ROW_HOVER_ALPHA: u8 = 26;
    const ASSET_NAME_DISPLAY_LEN: usize = 24;
    const MAX_TRANSACTION_ASSET_SUGGESTIONS: usize = 6;
    const REQUEST_POLL_INTERVAL: Duration = Duration::from_millis(100);
    const SQUIRREL_IMG_PATH: &str = "img/squirrel_68x68.png";

    pub fn new(backend: BACKEND) -> eyre::Result<Self> {
        let mut app = Self {
            backend,
            squirrel_texture: None,
            request_data: RequestData::default(),
            page: Page::MainMenu,
            allocation_record_date: Zoned::now().date(),
            allocation_record_assets: Vec::new(),
            message: None,
            alloc_diagram_category_id: None,
            alloc_diagram_data: None,
            categories: Vec::new(),
            asset_name_by_id: HashMap::new(),
            cfg_catgs_input: ConfigureCatgoriesInput::default(),
            pending_req_cnt: 0,
            latest_record: None,
            add_asset_args: AddAssetArgs::default(),
            log_buy_transaction_input: LogBuyTransactionInput::default(),
            listed_transactions: Vec::new(),
            transaction_asset_isins_input: String::new(),
            transaction_assets: Vec::new(),
            portfolio_overview_items: Vec::new(),
            portfolio_isin_items: Vec::new(),
            portfolio_isin: None,
            portfolio_asset_name: None,
            log_sell_transaction_input: LogSellTransactionInput::default(),
        };
        app.start_load_png_data(Self::SQUIRREL_IMG_PATH.to_string());
        app.start_get_categories();
        app.start_get_latest_record();
        Ok(app)
    }

    with_gui_requests!(implement_requests);

    fn decr_pending_req_cnt(&mut self) {
        if self.pending_req_cnt > 0 {
            self.pending_req_cnt -= 1;
        } else {
            self.message = Some("Failed to decrease pending request counter".to_string());
        }
    }

    fn incr_pending_req_cnt(&mut self) {
        self.pending_req_cnt += 1;
    }

    fn allocation_diagram_category_selected_text(&self) -> &str {
        match self.alloc_diagram_category_id {
            Some(category_id) => self
                .categories
                .iter()
                .find(|category| category.id == category_id)
                .map(|category| category.name.as_str())
                .unwrap_or("Position"),
            None => "Position",
        }
    }

    fn show_allocation_diagram_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Allocation Diagram", |app| {
            app.page = Page::MainMenu;
        });

        let prev_category_id = self.alloc_diagram_category_id;
        egui::ComboBox::from_id_salt("allocation_diagram_category")
            .selected_text(self.allocation_diagram_category_selected_text())
            .show_ui(ui, |ui| {
                for category in &self.categories {
                    ui.selectable_value(
                        &mut self.alloc_diagram_category_id,
                        Some(category.id),
                        &category.name,
                    );
                }
                ui.selectable_value(&mut self.alloc_diagram_category_id, None, "Position");
            });
        ui.add_space(Self::SPACE_2);

        if prev_category_id != self.alloc_diagram_category_id {
            if let Some(category_id) = self.alloc_diagram_category_id {
                self.start_get_alloc_diagram_data(GetAllocDiagramDataArgs {
                    category_id,
                    days: 5,
                });
            } else {
                self.alloc_diagram_data = None;
            }
            self.start_get_latest_record();
        }
        if let Some(data) = self.alloc_diagram_data.as_ref() {
            draw_percent_stacked_bar_chart(ui, data);
        } else if let Some(record) = &self.latest_record {
            let total: f64 = record.positions.iter().map(|p| p.amount).sum();

            if total <= 0. {
                ui.label("The latest allocation record contains no positive positions.");
                return;
            }

            ui.label(format!("Record from {}:", record.date));
            ui.add_space(10.0);

            for position in &record.positions {
                let percentage = (position.amount / total) * 100.0;
                let fraction = position.amount as f32 / total as f32;

                ui.label(format!(
                    "{} - {} ({:.1}%)",
                    position.asset.name, position.amount, percentage
                ));

                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(320.0)
                        .text(format!("{:.1}%", percentage)),
                );

                ui.add_space(6.0);
            }
        }
    }

    fn show_page_button(
        &mut self,
        ui: &mut egui::Ui,
        page: Page,
        label: &str,
        init_page_fn: fn(&mut Self) -> eyre::Result<()>,
    ) {
        let selected = self.page == page
            || (page == Page::Transactions
                && matches!(
                    self.page,
                    Page::LogBuyTransaction | Page::LogSellTransaction
                ))
            || (page == Page::TransactionAssets
                && matches!(self.page, Page::ImportTransactionAssets));
        let response = ui.add_sized(
            [180.0, 20.0],
            egui::Button::selectable(selected, label).right_text(""),
        );
        if response.clicked() {
            match init_page_fn(self) {
                Ok(_) => {
                    self.page = page;
                }
                Err(e) => {
                    self.message = Some(e.to_string());
                }
            }
        }
    }

    fn show_back_header(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        on_back: impl FnOnce(&mut Self),
    ) {
        let mut go_back = false;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_space(3.0);
                if ui
                    .add_sized(
                        [Self::BACK_BTN_SIZE, Self::BACK_BTN_SIZE],
                        egui::Button::new(egui::RichText::new("‹").size(Self::BACK_BTN_FONT_SIZE)),
                    )
                    .clicked()
                {
                    go_back = true;
                }
            });
            ui.add_space(Self::BACK_BTN_RMARGIN);
            ui.label(egui::RichText::new(title).heading().size(Self::H2_SIZE));
        });
        ui.add_space(Self::SPACE_3);
        if go_back {
            on_back(self);
        }
    }

    fn go_to_transactions_page(&mut self) {
        match self.init_transactions_page() {
            Ok(()) => {
                self.page = Page::Transactions;
            }
            Err(err) => {
                self.message = Some(err.to_string());
            }
        }
    }

    fn go_to_transaction_assets_page(&mut self) {
        match self.init_transaction_assets_page() {
            Ok(()) => {
                self.page = Page::TransactionAssets;
            }
            Err(err) => {
                self.message = Some(err.to_string());
            }
        }
    }

    fn reset_add_allocation_record_page(&mut self) {
        self.allocation_record_date = Zoned::now().date();
        for asset in &mut self.allocation_record_assets {
            asset.amount.clear();
        }
    }

    fn init_add_allocation_record_page(&mut self) -> eyre::Result<()> {
        self.reset_add_allocation_record_page();
        self.start_get_assets();
        self.message = None;
        Ok(())
    }

    fn init_configure_categories_page(&mut self) -> eyre::Result<()> {
        self.start_get_categories();
        self.message = None;
        Ok(())
    }

    fn init_alocation_diagram_page(&mut self) -> eyre::Result<()> {
        Ok(())
    }

    fn reset_log_buy_transaction_page(&mut self) {
        self.log_buy_transaction_input = LogBuyTransactionInput::default();
    }

    fn init_log_buy_transaction_page(&mut self) -> eyre::Result<()> {
        self.reset_log_buy_transaction_page();
        self.start_list_transaction_assets();
        self.message = None;
        Ok(())
    }

    fn init_transactions_page(&mut self) -> eyre::Result<()> {
        self.start_list_transactions();
        self.message = None;
        Ok(())
    }

    fn init_transaction_assets_page(&mut self) -> eyre::Result<()> {
        self.start_list_transaction_assets();
        self.message = None;
        Ok(())
    }

    fn init_import_transaction_assets_page(&mut self) -> eyre::Result<()> {
        self.transaction_asset_isins_input.clear();
        self.message = None;
        Ok(())
    }

    fn init_log_sell_transaction_page(&mut self) -> eyre::Result<()> {
        self.portfolio_isin = None;
        self.portfolio_asset_name = None;
        self.portfolio_isin_items.clear();
        self.reset_portfolio_sale_inputs();
        self.start_list_portfolio_overview_items();
        self.message = None;
        Ok(())
    }

    fn reset_portfolio_sale_inputs(&mut self) {
        self.log_sell_transaction_input = LogSellTransactionInput::default();
    }

    fn reset_add_asset_page(&mut self) {
        self.add_asset_args = AddAssetArgs::default();
    }

    fn init_add_asset_page(&mut self) -> eyre::Result<()> {
        self.reset_add_asset_page();
        self.start_get_categories();
        self.message = None;
        Ok(())
    }

    fn show_help_if_any(ui: &mut egui::Ui, label: &str, help_text: Option<&str>) {
        if let Some(help_text) = help_text {
            let help_id = format!("{}_help", label);
            let response = ui.add_sized(
                [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                egui::Label::new(egui::RichText::new("?").color(ui.visuals().hyperlink_color))
                    .sense(egui::Sense::click()),
            );
            egui::Popup::menu(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .width(Self::HELP_POPUP_WIDTH)
                .id(ui.make_persistent_id(help_id))
                .show(|ui| {
                    ui.label(help_text);
                });
        } else {
            ui.label("");
        }
    }

    fn show_widget_input_row(
        ui: &mut egui::Ui,
        label: &str,
        widget: impl Widget,
        help_text: Option<&str>,
    ) {
        ui.label(format!("{label}:"));
        ui.add_sized(
            [Self::DEFAULT_INPUT_WIDTH, Self::DEFAULT_INPUT_HEIGHT],
            widget,
        );
        Self::show_help_if_any(ui, label, help_text);
        ui.end_row();
    }

    fn show_enum_input_row<T>(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut T,
        help_text: Option<&str>,
    ) where
        T: IntoEnumIterator + Copy + PartialEq + Display,
    {
        ui.label(format!("{label}:"));
        egui::ComboBox::from_id_salt(format!("{}_combobox", label))
            .selected_text(value.to_string())
            .width(Self::DEFAULT_INPUT_WIDTH)
            .height(Self::DEFAULT_INPUT_HEIGHT)
            .show_ui(ui, |ui| {
                for enum_value in T::iter() {
                    ui.selectable_value(value, enum_value, enum_value.to_string());
                }
            });
        Self::show_help_if_any(ui, label, help_text);
        ui.end_row();
    }

    fn format_decimal_for_display(value: &str) -> String {
        let Some((integer, fraction)) = value.split_once('.') else {
            return value.to_string();
        };

        let fraction = &fraction[..fraction
            .len()
            .min(Self::DECIMAL_DISPLAY_MAX_FRACTION_DIGITS)];
        let fraction = fraction.trim_end_matches('0');

        if fraction.is_empty() {
            integer.to_string()
        } else {
            format!("{integer}.{fraction}")
        }
    }

    fn show_add_asset_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Add Asset", |app| {
            app.page = Page::MainMenu;
        });

        ui.label("Name:");
        ui.text_edit_singleline(&mut self.add_asset_args.name);
        ui.add_space(Self::SPACE_2);

        ui.label("Reference type:");
        egui::ComboBox::from_id_salt("reference_type")
            .selected_text(self.add_asset_args.reference.r#type.to_string())
            .show_ui(ui, |ui| {
                for t in AssetReferenceType::iter() {
                    ui.selectable_value(
                        &mut self.add_asset_args.reference.r#type,
                        t,
                        t.to_string(),
                    );
                }
            });
        ui.add_space(Self::SPACE_2);

        ui.label("Reference value:");
        ui.text_edit_singleline(&mut self.add_asset_args.reference.value);
        ui.vertical(|ui| {
            for catgy in &mut self.categories {
                let assignments = self
                    .add_asset_args
                    .category_id_to_assignment
                    .entry(catgy.id)
                    .or_default();

                ui.add_space(Self::SPACE_2);
                ui.horizontal(|ui| {
                    if assignments.len() < catgy.values.len()
                        && ui
                            .add_sized(
                                [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                egui::Button::new("+"),
                            )
                            .clicked()
                    {
                        assignments.push(CategoryAssignmentPc {
                            percentage: if assignments.is_empty() { 100. } else { 0. },
                            value_id: None,
                        });
                    }
                    ui.label(format!(" {}:", &catgy.name));
                });
                ui.add_space(Self::SPACE_1);

                let mut del_assignm_idx: Option<usize> = None;
                for assignm_idx in (0..assignments.len()).rev() {
                    let assignment = &mut assignments[assignm_idx];
                    let selected_text = assignment
                        .value_id
                        .and_then(|id| catgy.values.iter().find(|val| val.id == id))
                        .map(|val| val.name.clone())
                        .unwrap_or_else(|| "Select...".to_string());

                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [70.0, Self::DEFAULT_INPUT_HEIGHT],
                            egui::DragValue::new(&mut assignment.percentage)
                                .range(0.0..=100.0)
                                .speed(0.1)
                                .fixed_decimals(2)
                                .suffix("%"),
                        );
                        egui::ComboBox::from_id_salt(format!("{}:{}", catgy.id, assignm_idx))
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for value in catgy.values.iter() {
                                    ui.selectable_value(
                                        &mut assignment.value_id,
                                        Some(value.id),
                                        &value.name,
                                    );
                                }
                            });
                        if ui
                            .add_sized(
                                [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                egui::Button::new("-"),
                            )
                            .clicked()
                        {
                            del_assignm_idx = Some(assignm_idx);
                        }
                    });
                }
                if let Some(idx) = del_assignm_idx {
                    assignments.remove(idx);
                }
            }
        });
        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            self.start_add_asset(self.add_asset_args.clone());
        }
    }

    fn show_log_buy_transaction_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Log a Purchase", |app| {
            app.go_to_transactions_page();
        });

        egui::Grid::new("log_buy_transaction_input_grid")
            .num_columns(3)
            .spacing([Self::SPACE_2, Self::SPACE_2])
            .show(ui, |ui| {
                Self::show_widget_input_row(
                    ui,
                    "Date",
                    DatePickerButton::new(&mut self.log_buy_transaction_input.date),
                    None,
                );
                Self::show_enum_input_row(
                    ui,
                    "Currency",
                    &mut self.log_buy_transaction_input.currency,
                    None,
                );
                self.show_transaction_asset_isin_input_row(ui);
                Self::show_widget_input_row(
                    ui,
                    "Quantity",
                    TextEdit::singleline(&mut self.log_buy_transaction_input.quantity),
                    None,
                );
                Self::show_widget_input_row(
                    ui,
                    "Share price",
                    TextEdit::singleline(&mut self.log_buy_transaction_input.share_price),
                    Some("The price per share or unit at which the asset was bought."),
                );
                Self::show_widget_input_row(
                    ui,
                    "Order value",
                    TextEdit::singleline(&mut self.log_buy_transaction_input.order_value),
                    Some("The total value of the buy order including fees and taxes."),
                );
            });
        ui.add_space(Self::SPACE_2);

        if ui.button("Save").clicked() {
            let mut input = self.log_buy_transaction_input.clone();
            input.client_today = Zoned::now().date();
            self.start_log_buy_transaction(input);
        }
    }

    fn show_transaction_asset_isin_input_row(&mut self, ui: &mut egui::Ui) {
        ui.label("ISIN:");
        ui.add_sized(
            [Self::DEFAULT_INPUT_WIDTH, Self::DEFAULT_INPUT_HEIGHT],
            TextEdit::singleline(&mut self.log_buy_transaction_input.isin),
        );
        Self::show_help_if_any(ui, "ISIN", None);
        ui.end_row();

        let suggestions = self.transaction_asset_suggestions();
        if suggestions.is_empty() {
            return;
        }

        let mut selected_isin = None;
        ui.label("");
        let suggestion_height = suggestions.len() as f32 * Self::DEFAULT_INPUT_HEIGHT;
        let (_, rect) = ui.allocate_space(egui::vec2(Self::DEFAULT_INPUT_WIDTH, suggestion_height));
        let suggestion_rect = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(
                Self::DEFAULT_INPUT_WIDTH + Self::SPACE_2 + Self::SYM_BTN_SIZE,
                suggestion_height,
            ),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(suggestion_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                for asset in suggestions {
                    let label = Self::transaction_asset_suggestion_label(asset);
                    if ui.link(label).clicked() {
                        selected_isin = Some(asset.isin.clone());
                    }
                }
            },
        );
        ui.allocate_space(egui::vec2(Self::SYM_BTN_SIZE, suggestion_height));
        ui.end_row();

        if let Some(isin) = selected_isin {
            self.log_buy_transaction_input.isin = isin;
        }
    }

    fn transaction_asset_suggestions(&self) -> Vec<&TransactionAsset> {
        let query = self.log_buy_transaction_input.isin.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        self.transaction_assets
            .iter()
            .filter(|asset| Self::transaction_asset_matches_query(asset, &query))
            .take(Self::MAX_TRANSACTION_ASSET_SUGGESTIONS)
            .collect()
    }

    fn transaction_asset_matches_query(asset: &TransactionAsset, query: &str) -> bool {
        asset.isin.to_lowercase().contains(query)
            || asset
                .symbol
                .as_deref()
                .is_some_and(|symbol| symbol.to_lowercase().contains(query))
            || asset
                .name
                .as_deref()
                .is_some_and(|name| name.to_lowercase().contains(query))
    }

    fn transaction_asset_suggestion_label(asset: &TransactionAsset) -> String {
        let title = asset
            .name
            .as_deref()
            .or(asset.symbol.as_deref())
            .unwrap_or(&asset.isin);
        match asset.symbol.as_deref() {
            Some(symbol) if title != symbol => format!("{title} ({symbol}) - {}", asset.isin),
            _ => format!("{title} - {}", asset.isin),
        }
    }

    fn show_transactions_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Transactions", |app| {
            app.page = Page::MainMenu;
        });

        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    Self::TRANSACTION_ACTION_BTN_SIZE,
                    egui::Button::new(egui::RichText::new("Buy").size(Self::H3_SIZE)),
                )
                .clicked()
            {
                match self.init_log_buy_transaction_page() {
                    Ok(()) => {
                        self.page = Page::LogBuyTransaction;
                    }
                    Err(err) => {
                        self.message = Some(err.to_string());
                    }
                }
            }
            if ui
                .add_sized(
                    Self::TRANSACTION_ACTION_BTN_SIZE,
                    egui::Button::new(egui::RichText::new("Sell").size(Self::H3_SIZE)),
                )
                .clicked()
            {
                match self.init_log_sell_transaction_page() {
                    Ok(()) => {
                        self.page = Page::LogSellTransaction;
                    }
                    Err(err) => {
                        self.message = Some(err.to_string());
                    }
                }
            }
        });
        ui.add_space(Self::SPACE_3);

        egui::Grid::new("list_transactions_grid")
            .striped(true)
            .spacing(Self::TABLE_GRID_SPACING)
            .show(ui, |ui| {
                ui.strong("Qty / Date");
                ui.strong("Name / ISIN");
                ui.strong("Value / Price");
                ui.strong("Ccy");
                ui.end_row();

                for transaction in &self.listed_transactions {
                    let (quantity, color) = match transaction.r#type {
                        TransactionType::Buy => (
                            format!("+{}", transaction.quantity),
                            Self::BUY_QUANTITY_COLOR,
                        ),
                        TransactionType::Sell => (
                            format!("-{}", transaction.quantity),
                            Self::SELL_QUANTITY_COLOR,
                        ),
                    };
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(quantity).color(color).strong());
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(&transaction.date);
                    });
                    ui.vertical(|ui| {
                        let asset_name = value_or_unknown(transaction.asset_name.as_deref());
                        let response = ui.label(
                            egui::RichText::new(trunc_str(
                                asset_name,
                                Self::ASSET_NAME_DISPLAY_LEN,
                            ))
                            .strong(),
                        );
                        if asset_name != UNKNOWN_STR {
                            response.on_hover_text(asset_name);
                        }
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(&transaction.isin);
                    });
                    ui.vertical(|ui| {
                        ui.label(&transaction.order_value);
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(&transaction.share_price);
                    });
                    ui.label(&transaction.currency);
                    ui.end_row();
                }
            });
    }

    fn show_transaction_assets_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Transaction Assets", |app| {
            app.page = Page::MainMenu;
        });

        if ui
            .add_sized(
                Self::TRANSACTION_ACTION_BTN_SIZE,
                egui::Button::new(egui::RichText::new("Import").size(Self::H3_SIZE)),
            )
            .clicked()
        {
            match self.init_import_transaction_assets_page() {
                Ok(()) => {
                    self.page = Page::ImportTransactionAssets;
                }
                Err(err) => {
                    self.message = Some(err.to_string());
                }
            }
        }
        ui.add_space(Self::SPACE_3);

        egui::Grid::new("transaction_assets_grid")
            .striped(true)
            .spacing(Self::TRANSACTION_ASSETS_GRID_SPACING)
            .show(ui, |ui| {
                ui.strong("Name / Updated");
                ui.strong("ISIN / Symbol");
                ui.strong("Type / Exchange");
                ui.end_row();

                for asset in &self.transaction_assets {
                    let asset_name = value_or_unknown(asset.name.as_deref());
                    ui.vertical(|ui| {
                        let response = ui.label(
                            egui::RichText::new(trunc_str(
                                asset_name,
                                Self::ASSET_NAME_DISPLAY_LEN,
                            ))
                            .strong(),
                        );
                        if asset_name != UNKNOWN_STR {
                            response.on_hover_text(asset_name);
                        }
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(format_updated_at(
                            asset.updated_at_date.as_deref(),
                            asset.updated_at_time.as_deref(),
                        ));
                    });
                    ui.vertical(|ui| {
                        ui.label(value_or_unknown(Some(asset.isin.as_str())));
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(value_or_unknown(asset.symbol.as_deref()));
                    });
                    ui.vertical(|ui| {
                        ui.label(value_or_unknown(asset.quote_type.as_deref()));
                        ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                        ui.label(value_or_unknown(asset.exchange.as_deref()));
                    });
                    ui.end_row();
                }
            });
    }

    fn show_import_transaction_assets_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Import Assets", |app| {
            app.go_to_transaction_assets_page();
        });

        ui.label("ISINs:");
        ui.horizontal(|ui| {
            ui.add_sized(
                [Self::DEFAULT_INPUT_WIDTH, 120.0],
                TextEdit::multiline(&mut self.transaction_asset_isins_input),
            );
            Self::show_help_if_any(
                ui,
                "Transaction asset ISINs",
                Some(
                    "Enter one or more ISINs separated by line breaks, spaces, commas, or semicolons. Each ISIN is validated and duplicates are ignored. Import looks up matching asset metadata via yahoo finance and saves it locally for later use in the app. Updates stored data if an entered asset already exists.",
                ),
            );
        });
        ui.add_space(Self::SPACE_2);

        if ui.button("Import").clicked() {
            let isins = self
                .transaction_asset_isins_input
                .lines()
                .map(ToOwned::to_owned)
                .collect();
            self.start_import_transaction_assets(core_lib::ImportTransactionAssetsInput { isins });
        }
    }

    fn show_log_sell_transaction_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Log a Sale", |app| {
            if app.portfolio_isin.is_some() {
                app.portfolio_isin = None;
                app.portfolio_asset_name = None;
                app.portfolio_isin_items.clear();
                app.reset_portfolio_sale_inputs();
                app.start_list_portfolio_overview_items();
            } else {
                app.go_to_transactions_page();
            }
        });

        if let Some(isin) = self.portfolio_isin.clone() {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(value_or_unknown(self.portfolio_asset_name.as_deref()))
                        .heading()
                        .size(Self::H3_SIZE)
                        .strong(),
                );
                ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                ui.label(&isin);
            });
            ui.add_space(Self::SPACE_3);

            if self.portfolio_isin_items.is_empty() {
                ui.label("No open portfolio items.");
                return;
            }

            egui::Grid::new("portfolio_items_grid")
                .striped(true)
                .spacing(Self::TABLE_GRID_SPACING)
                .show(ui, |ui| {
                    ui.strong("Qty / Date");
                    ui.strong("Value / Price");
                    ui.strong("Ccy");
                    ui.strong("Sell Qty");
                    ui.end_row();

                    for item in &self.portfolio_isin_items {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "+{}",
                                    Self::format_decimal_for_display(&item.quantity)
                                ))
                                .color(Self::BUY_QUANTITY_COLOR)
                                .strong(),
                            );
                            ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                            ui.label(&item.buy_date);
                        });
                        ui.vertical(|ui| {
                            ui.label(Self::format_decimal_for_display(&item.order_value));
                            ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                            ui.label(Self::format_decimal_for_display(&item.share_price));
                        });
                        ui.label(&item.currency);
                        let quantity = self
                            .log_sell_transaction_input
                            .portfolio_item_id_to_quantity
                            .entry(item.portfolio_item_id)
                            .or_default();
                        ui.add_sized(
                            [80.0, Self::DEFAULT_INPUT_HEIGHT],
                            TextEdit::singleline(quantity),
                        );
                        ui.end_row();
                    }
                });

            ui.add_space(Self::SPACE_3);
            egui::Grid::new("portfolio_sale_input_grid")
                .num_columns(3)
                .spacing([Self::SPACE_2, Self::SPACE_2])
                .show(ui, |ui| {
                    Self::show_widget_input_row(
                        ui,
                        "Date",
                        DatePickerButton::new(&mut self.log_sell_transaction_input.date),
                        None,
                    );
                    Self::show_widget_input_row(
                        ui,
                        "Share price",
                        TextEdit::singleline(&mut self.log_sell_transaction_input.share_price),
                        None,
                    );
                    Self::show_widget_input_row(
                        ui,
                        "Order value",
                        TextEdit::singleline(&mut self.log_sell_transaction_input.order_value),
                        None,
                    );
                    Self::show_enum_input_row(
                        ui,
                        "Currency",
                        &mut self.log_sell_transaction_input.currency,
                        None,
                    );
                });

            ui.add_space(Self::SPACE_3);
            if ui.button("Save").clicked() {
                self.log_sell_transaction_input.isin = isin.clone();
                self.log_sell_transaction_input.client_today = Zoned::now().date();
                self.log_sell_transaction_input
                    .portfolio_item_id_to_quantity
                    .retain(|_, quantity| !quantity.trim().is_empty());
                self.start_log_sell_transaction(self.log_sell_transaction_input.clone());
            }
            return;
        }

        if self.portfolio_overview_items.is_empty() {
            ui.label("No open positions to sell.");
            return;
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Select a Position...")
                    .heading()
                    .size(Self::H3_SIZE),
            );
        });
        ui.add_space(Self::SPACE_3);

        let mut selected_position: Option<(String, Option<String>)> = None;
        egui::Grid::new("portfolio_positions_grid")
            .striped(true)
            .spacing(Self::TABLE_GRID_SPACING)
            .show(ui, |ui| {
                ui.strong("Name / ISIN");
                ui.strong("Qty");
                ui.strong("Value / Price");
                ui.strong("Ccy");
                ui.end_row();

                for position in &self.portfolio_overview_items {
                    let mut row_rect = egui::Rect::NOTHING;
                    let name_response = ui
                        .vertical(|ui| {
                            let asset_name = value_or_unknown(position.asset_name.as_deref());
                            let response = ui.label(
                                egui::RichText::new(trunc_str(
                                    asset_name,
                                    Self::ASSET_NAME_DISPLAY_LEN,
                                ))
                                .strong(),
                            );
                            if asset_name != UNKNOWN_STR {
                                response.clone().on_hover_text(asset_name);
                            }
                            ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                            ui.label(&position.isin);
                        })
                        .response;
                    row_rect = row_rect.union(name_response.rect);

                    let quantity_response =
                        ui.label(Self::format_decimal_for_display(&position.quantity));
                    row_rect = row_rect.union(quantity_response.rect);

                    let value_response = ui
                        .vertical(|ui| {
                            ui.label(Self::format_decimal_for_display(&position.total_value));
                            ui.add_space(Self::TRANSACTION_ASSET_ROW_LINE_SPACING);
                            ui.label(Self::format_decimal_for_display(
                                &position.average_share_price,
                            ));
                        })
                        .response;
                    row_rect = row_rect.union(value_response.rect);

                    let currency_response = ui.label(&position.currency);
                    row_rect = row_rect.union(currency_response.rect);

                    let mut row_rect = row_rect.expand2(egui::vec2(
                        Self::TABLE_GRID_SPACING[0] / 2.0,
                        Self::TABLE_GRID_SPACING[1] / 2.0,
                    ));
                    row_rect.max.x += 10.0;
                    let row_response = ui
                        .interact(
                            row_rect,
                            ui.make_persistent_id(("portfolio_position", &position.isin)),
                            egui::Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if row_response.hovered() {
                        ui.painter().rect_filled(
                            row_rect,
                            egui::CornerRadius::ZERO,
                            egui::Color32::from_white_alpha(Self::CLICKABLE_ROW_HOVER_ALPHA),
                        );
                    }
                    if row_response.clicked() {
                        selected_position =
                            Some((position.isin.clone(), position.asset_name.clone()));
                    }
                    ui.end_row();
                }
            });

        if let Some((isin, asset_name)) = selected_position {
            self.portfolio_isin = Some(isin.clone());
            self.portfolio_asset_name = asset_name;
            self.portfolio_isin_items.clear();
            self.reset_portfolio_sale_inputs();
            self.log_sell_transaction_input.isin = isin.clone();
            self.start_list_portfolio_isin_items(isin);
        }
    }

    fn show_configure_categories_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Configure Categories", |app| {
            app.page = Page::MainMenu;
        });
        if ui.button("Save").clicked() {
            self.start_configure_categories(self.cfg_catgs_input.clone());
        }
        ui.add_space(Self::SPACE_2);
        let mut focus_next_catg_input = false;
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                    egui::Button::new("+"),
                )
                .clicked()
            {
                self.cfg_catgs_input
                    .new_category_inputs
                    .push(NewCategoryInput::default());
                focus_next_catg_input = true;
            }
            ui.label("Categories:");
        });

        /* Show inputs for new categories */
        let mut del_catg_idx: Option<usize> = None;
        for (catg_idx, catg_input) in self
            .cfg_catgs_input
            .new_category_inputs
            .iter_mut()
            .enumerate()
            .rev()
        {
            ui.add_space(Self::SPACE_2);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("•");
                    let id = ui.make_persistent_id(("NewCatg", catg_idx));
                    let response = ui.add(TextEdit::singleline(&mut catg_input.name).id(id));
                    if focus_next_catg_input {
                        response.request_focus();
                        focus_next_catg_input = false;
                    }
                    if ui
                        .add_sized(
                            [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                            egui::Button::new("-"),
                        )
                        .clicked()
                    {
                        del_catg_idx = Some(catg_idx);
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(Self::SPACE_3);
                    ui.vertical(|ui| {
                        let mut focus_next_val_input = false;
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                    egui::Button::new("+"),
                                )
                                .clicked()
                            {
                                catg_input
                                    .new_value_inputs
                                    .push(CategoryValueInput::default());
                                focus_next_val_input = true;
                            }
                            ui.label("Values:");
                        });

                        /* Show inputs for new values */
                        let mut del_val_idx: Option<usize> = None;
                        for (val_idx, val_input) in
                            catg_input.new_value_inputs.iter_mut().enumerate().rev()
                        {
                            ui.horizontal(|ui| {
                                ui.label("•");
                                let id = ui.make_persistent_id(("NewCatgVal", catg_idx, val_idx));
                                let response =
                                    ui.add(TextEdit::singleline(&mut val_input.name).id(id));
                                if focus_next_val_input {
                                    response.request_focus();
                                    focus_next_val_input = false;
                                }
                                if ui
                                    .add_sized(
                                        [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                        egui::Button::new("-"),
                                    )
                                    .clicked()
                                {
                                    del_val_idx = Some(val_idx);
                                }
                            });
                        }
                        if let Some(idx) = del_val_idx {
                            catg_input.new_value_inputs.remove(idx);
                        }
                    });
                });
            });
        }
        if let Some(idx) = del_catg_idx {
            self.cfg_catgs_input.new_category_inputs.remove(idx);
        }

        /* Show existing categories */
        for catg in &self.categories {
            let catg_input = &mut self
                .cfg_catgs_input
                .category_id_to_adapt_input
                .entry(catg.id)
                .or_default();

            ui.add_space(Self::SPACE_2);
            ui.label(format!("• {}", catg.name));
            ui.horizontal(|ui| {
                ui.add_space(Self::SPACE_3);
                ui.vertical(|ui| {
                    let mut focus_next_val_input = false;
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                egui::Button::new("+"),
                            )
                            .clicked()
                        {
                            catg_input
                                .new_value_inputs
                                .push(CategoryValueInput::default());
                            focus_next_val_input = true;
                        }
                        ui.label("Values:");
                    });

                    /* Show inputs for new values */
                    let mut del_val_idx: Option<usize> = None;
                    for (val_idx, val_input) in
                        catg_input.new_value_inputs.iter_mut().enumerate().rev()
                    {
                        ui.horizontal(|ui| {
                            ui.label("•");
                            let id = ui.make_persistent_id(("AdaptCatgVal", catg.id, val_idx));
                            let response = ui.add(TextEdit::singleline(&mut val_input.name).id(id));
                            if focus_next_val_input {
                                response.request_focus();
                                focus_next_val_input = false;
                            }
                            if ui
                                .add_sized(
                                    [Self::SYM_BTN_SIZE, Self::SYM_BTN_SIZE],
                                    egui::Button::new("-"),
                                )
                                .clicked()
                            {
                                del_val_idx = Some(val_idx);
                            }
                        });
                    }
                    if let Some(idx) = del_val_idx {
                        catg_input.new_value_inputs.remove(idx);
                    }

                    /* Show existing values */
                    for value in &catg.values {
                        ui.label(format!("• {}", value.name));
                    }
                });
            });
        }
    }

    fn show_add_allocation_record_page(&mut self, ui: &mut egui::Ui) {
        self.show_back_header(ui, "Add Allocation Record", |app| {
            app.page = Page::MainMenu;
        });

        ui.label("Date:");
        ui.add(DatePickerButton::new(&mut self.allocation_record_date));

        ui.add_space(Self::SPACE_2);
        ui.label("Positions:");

        ui.vertical(|ui| {
            for asset in &mut self.allocation_record_assets {
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut asset.amount).desired_width(80.0));
                    ui.label(&asset.label);
                });
            }
        });

        ui.add_space(Self::SPACE_2);
        if ui.button("Save").clicked() {
            let mut positions = Vec::new();
            let mut validation_error = None;

            for asset in &self.allocation_record_assets {
                let trimmed = asset.amount.trim();

                if trimmed.is_empty() {
                    continue;
                }

                let amount = match trimmed.parse::<f64>() {
                    Ok(value) => value,
                    Err(_) => {
                        validation_error =
                            Some(format!("Invalid amount for asset '{}'", asset.label));
                        break;
                    }
                };

                if amount <= 0. {
                    validation_error = Some(format!(
                        "Amount must be greater than 0 for asset '{}'",
                        asset.label
                    ));
                    break;
                }

                positions.push(AllocationPositionInput {
                    asset_id: asset.asset_id,
                    amount,
                });
            }

            if let Some(message) = validation_error {
                self.message = Some(message);
            } else {
                /*
                match self.asset_service.add_allocation_record(
                    self.allocation_record_date,
                    positions,
                ) {
                    Ok(()) => {
                        self.message = Some(format!(
                            "Allocation record '{}' was saved.",
                            self.allocation_record_date.to_string()
                        ));
                        self.reset_add_allocation_record_page();
                    }
                    Err(err) => {
                        self.message = Some(err.to_string());
                    }
                }
                */
            }
        }
    }

    fn poll_request_receivers(&mut self, ui: &mut egui::Ui) {
        if let Some(data) = self.poll_load_png_data_rx() {
            match load_png_texture_from_bytes(ui.ctx(), Self::SQUIRREL_IMG_PATH, data) {
                Ok(texture) => {
                    self.squirrel_texture = Some(texture);
                }
                Err(err) => {
                    self.squirrel_texture = None;
                    self.message = Some(err.to_string());
                }
            }
        }
        if let Some(categories) = self.poll_get_categories_rx() {
            self.categories = categories;
        }
        if let Some(assets) = self.poll_get_assets_rx() {
            self.asset_name_by_id.clear();
            self.allocation_record_assets.clear();
            for asset in assets {
                self.asset_name_by_id.insert(asset.id, asset.name.clone());
                self.allocation_record_assets.push(PositionItem {
                    asset_id: asset.id,
                    label: format!("{} ({})", asset.name, asset.reference.value),
                    amount: String::new(),
                });
            }
        }
        if let Some(record) = self.poll_get_latest_record_rx() {
            self.latest_record = record;
        }
        if let Some(data) = self.poll_get_alloc_diagram_data_rx() {
            self.alloc_diagram_data = Some(data);
        }
        if let Some((categories, err)) = self.poll_configure_categories_rx() {
            if let Some(err) = err {
                self.message = Some(format!("Partial save. First error: {}", err));
            } else {
                self.message = Some("All saved".into());
            }
            self.cfg_catgs_input = categories;
            self.start_get_categories();
        }
        self.poll_add_asset_rx();
        if self.poll_log_buy_transaction_rx().is_some() {
            self.message = Some("Buy transaction logged".into());
            self.reset_log_buy_transaction_page();
        }
        if self.poll_log_sell_transaction_rx().is_some() {
            self.message = Some("Sale logged".into());
            self.reset_portfolio_sale_inputs();
            if let Some(isin) = self.portfolio_isin.clone() {
                self.start_list_portfolio_isin_items(isin);
            } else {
                self.start_list_portfolio_overview_items();
            }
        }
        if let Some(transactions) = self.poll_list_transactions_rx() {
            self.listed_transactions = transactions;
        }
        if let Some(assets) = self.poll_import_transaction_assets_rx() {
            self.transaction_assets = assets;
            self.transaction_asset_isins_input.clear();
            self.message = Some("Transaction assets imported".into());
        }
        if let Some(assets) = self.poll_list_transaction_assets_rx() {
            self.transaction_assets = assets;
        }
        if let Some(positions) = self.poll_list_portfolio_overview_items_rx() {
            self.portfolio_overview_items = positions;
        }
        if let Some(items) = self.poll_list_portfolio_isin_items_rx() {
            self.portfolio_isin_items = items;
        }
    }

    fn show_main_menu_page(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            self.show_page_button(
                ui,
                Page::AllocationDiagram,
                "Allocation Diagram",
                Self::init_alocation_diagram_page,
            );
            self.show_page_button(ui, Page::AddAsset, "Add Asset", Self::init_add_asset_page);
            self.show_page_button(
                ui,
                Page::ConfigureCategories,
                "Configure Categories",
                Self::init_configure_categories_page,
            );
            self.show_page_button(
                ui,
                Page::AddAllocationRecord,
                "Add Allocation Record",
                Self::init_add_allocation_record_page,
            );
            self.show_page_button(
                ui,
                Page::Transactions,
                "Transactions",
                Self::init_transactions_page,
            );
            self.show_page_button(
                ui,
                Page::TransactionAssets,
                "Transaction Assets",
                Self::init_transaction_assets_page,
            );
        });
    }

    fn show_content(&mut self, ui: &mut egui::Ui) {
        ui.add_space(Self::SPACE_2);
        ui.horizontal(|ui| {
            if let Some(texture) = &self.squirrel_texture
                && ui
                    .add(
                        egui::Image::new((texture.id(), egui::vec2(68.0, 68.0)))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
            {
                self.page = Page::MainMenu;
            }
            ui.add_space(Self::SPACE_2);
            ui.label(
                egui::RichText::new(APP_NAME.to_string())
                    .heading()
                    .size(Self::H1_SIZE),
            );
        });
        ui.add_space(Self::SPACE_3);
        ui.vertical(|ui| {
            if self.page == Page::MainMenu {
                self.show_main_menu_page(ui);
            } else if self.pending_req_cnt > 0 {
                ui.label("Loading...");
            } else {
                match self.page {
                    Page::MainMenu => self.show_main_menu_page(ui),
                    Page::AddAsset => self.show_add_asset_page(ui),
                    Page::AllocationDiagram => self.show_allocation_diagram_page(ui),
                    Page::ConfigureCategories => self.show_configure_categories_page(ui),
                    Page::AddAllocationRecord => self.show_add_allocation_record_page(ui),
                    Page::LogBuyTransaction => self.show_log_buy_transaction_page(ui),
                    Page::Transactions => self.show_transactions_page(ui),
                    Page::LogSellTransaction => self.show_log_sell_transaction_page(ui),
                    Page::TransactionAssets => self.show_transaction_assets_page(ui),
                    Page::ImportTransactionAssets => self.show_import_transaction_assets_page(ui),
                }
            }
            ui.add_space(Self::SPACE_3);
            ui.label(egui::RichText::new("Message").heading().size(Self::H2_SIZE));
            ui.add_space(Self::SPACE_2);
            if let Some(message) = &self.message {
                ui.colored_label(egui::Color32::RED, message);
            }
        });
    }
}

impl<B: AppBackend> eframe::App for EframeApp<B> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_request_receivers(ui);
        ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        self.show_content(ui);
                    });
                });
        });
        if self.pending_req_cnt > 0 {
            // Keep polling background request receivers even when there is no input-driven repaint
            ui.ctx().request_repaint_after(Self::REQUEST_POLL_INTERVAL);
        }
    }
}
