/// Macros that call a given macro with the canonical request API of the app backend
///
/// Request-list macro variants:
///
/// - `with_gui_requests!`: requests used by the GUI frontend of the app
/// - `with_nogui_requests!`: requests used only by external tools
///
/// Request declarations use this syntax:
///
/// ```text
/// #[access(<policy>)]
/// request_name(optional_argument_type) -> return_type;
/// ```
///
/// `request_name` is also the generated Rust method name and web route name. Requests may have
/// either no argument, written as `request_name()`, or exactly one owned, serialization-friendly
/// argument type, written as `request_name(Type)`. The return type must also be owned and
/// serialization-friendly because web requests cross the JSON boundary.
///
/// Each request must declare its web access policy explicitly:
///
/// - `#[access(Public)]` means the web backend exposes the request without an access token.
/// - `#[access(Token)]` means the web backend requires a valid access token for that request.
///
/// Keep these lists as the single source of truth for generated request plumbing.

#[macro_export]
macro_rules! with_gui_requests {
    ($macro:ident) => {
        $macro! {
            #[access(Public)]
            unlock(core_lib::UnlockPatternInput) -> core_lib::AccessGrant;
            #[access(Token)]
            get_categories() -> Vec<core_lib::Category>;
            #[access(Token)]
            get_assets() -> Vec<core_lib::Asset>;
            #[access(Token)]
            get_latest_record() -> Option<core_lib::AllocationRecord>;
            #[access(Token)]
            get_alloc_diagram_data(core_lib::GetAllocDiagramDataArgs) -> core_lib::AllocationDiagramData;
            #[access(Token)]
            add_asset(core_lib::AddAssetArgs) -> ();
            #[access(Token)]
            log_buy_transaction(core_lib::LogBuyTransactionInput) -> ();
            #[access(Token)]
            log_sell_transaction(core_lib::LogSellTransactionInput) -> ();
            #[access(Token)]
            list_transactions() -> Vec<core_lib::ListedTransaction>;
            #[access(Token)]
            import_transaction_assets(core_lib::ImportTransactionAssetsInput) -> Vec<core_lib::TransactionAsset>;
            #[access(Token)]
            list_transaction_assets() -> Vec<core_lib::TransactionAsset>;
            #[access(Token)]
            list_portfolio_overview_items() -> Vec<core_lib::PortfolioOverviewItem>;
            #[access(Token)]
            list_portfolio_isin_items(String) -> Vec<core_lib::PortfolioIsinItem>;
            #[access(Token)]
            load_png_data(String) -> Vec<u8>;
            #[access(Token)]
            configure_categories(core_lib::ConfigureCatgoriesInput) -> (core_lib::ConfigureCatgoriesInput, Option<String>);
        }
    };
}

#[macro_export]
macro_rules! with_nogui_requests {
    ($macro:ident) => {
        $macro! {
            #[access(Token)]
            create_data_backup() -> Vec<u8>;
        }
    };
}
