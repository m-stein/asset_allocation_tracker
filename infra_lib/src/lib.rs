use core_lib::{
    AdaptCategoryInput, AllocationRecord, Asset, AssetReference, AssetReferenceType,
    CategoryAssignment, ConfigureCatgoriesInput, Currency, GetAllocDiagramDataArgs,
    ImportTransactionAssetsInput, ListedTransaction, LogBuyTransactionInput,
    LogSellTransactionInput, NewCategoryInput, PortfolioIsinItem, PortfolioOverviewItem,
    TransactionAsset, TransactionType, add_asset_args::AddAssetArgs,
    allocation_diagram_data::AllocationDiagramData, category::Category,
    category_value::CategoryValue,
};
use eyre::eyre;
use flate2::{Compression, write::GzEncoder};
use jiff::civil::Time;
use rusqlite::{OptionalExtension, params, types::FromSqlError};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const TRANSACTIONS_DB_SCHEMA_VERSION: u64 = 1;
pub const ASSETS_DB_SCHEMA_VERSION: u64 = 1;
pub const ALLOCATION_RECORD_FORMAT_VERSION: u64 = 1;

const SCHEMA_VERSION_KEY: &str = "schema_version";
static DATA_BACKUP_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAllocationRecord {
    format_version: u64,
    record: AllocationRecord,
}

fn data_dir_path() -> PathBuf {
    env::var("TALLYTAIL_DATA_DIR")
        .map(PathBuf::from)
        .expect("TALLYTAIL_DATA_DIR must be set")
}

fn transactions_db_path() -> PathBuf {
    data_dir_path().join("transactions.sdb")
}

fn assets_db_path() -> PathBuf {
    data_dir_path().join("assets.sdb")
}

fn allocation_records_dir() -> PathBuf {
    data_dir_path().join("allocation_records")
}

fn ensure_data_dir() -> eyre::Result<()> {
    fs::create_dir_all(allocation_records_dir())?;
    Ok(())
}

fn open_transactions_connection() -> eyre::Result<rusqlite::Connection> {
    open_db_connection(
        transactions_db_path(),
        "transactions database",
        TRANSACTIONS_DB_SCHEMA_VERSION,
        initialize_transactions_schema,
    )
}

fn open_assets_connection() -> eyre::Result<rusqlite::Connection> {
    open_db_connection(
        assets_db_path(),
        "assets database",
        ASSETS_DB_SCHEMA_VERSION,
        initialize_assets_schema,
    )
}

fn open_db_connection(
    path: PathBuf,
    name: &str,
    schema_version: u64,
    initialize_schema: fn(&rusqlite::Connection) -> eyre::Result<()>,
) -> eyre::Result<rusqlite::Connection> {
    ensure_data_dir()?;
    let is_new_database = !path.exists();
    let connection = rusqlite::Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    if is_new_database {
        initialize_schema(&connection)?;
    } else {
        ensure_database_schema_version(&connection, name, schema_version)?;
    }
    Ok(connection)
}

pub fn get_alloc_diagram_data(
    args: GetAllocDiagramDataArgs,
) -> eyre::Result<AllocationDiagramData> {
    let records = get_latest_records(args.days as usize)?;
    let category_name = get_category_name_by_id(args.category_id)?;
    Ok(AllocationDiagramData::new(records, &category_name))
}

pub fn load_png_data(path: String) -> eyre::Result<Vec<u8>> {
    Ok(std::fs::read(path)?)
}

pub fn create_data_backup() -> eyre::Result<Vec<u8>> {
    let _backup_lock = DATA_BACKUP_LOCK
        .lock()
        .map_err(|_| eyre!("Data backup lock is poisoned"))?;
    ensure_data_dir()?;

    let temp_dir = tempfile::tempdir()?;
    let assets_backup_path = temp_dir.path().join("assets.sdb");
    let transactions_backup_path = temp_dir.path().join("transactions.sdb");

    backup_database(&open_assets_connection()?, &assets_backup_path)?;
    backup_database(&open_transactions_connection()?, &transactions_backup_path)?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    append_file_to_tar(&mut encoder, &assets_backup_path, "assets.sdb")?;
    append_file_to_tar(&mut encoder, &transactions_backup_path, "transactions.sdb")?;
    append_directory_to_tar(
        &mut encoder,
        &allocation_records_dir(),
        "allocation_records",
    )?;
    write_tar_end(&mut encoder)?;

    Ok(encoder.finish()?)
}

pub fn add_asset(args: AddAssetArgs) -> eyre::Result<()> {
    let name = args.name.trim();
    if name.is_empty() {
        return Err(eyre!("Asset name must not be empty"));
    }
    if args.reference.value.is_empty() {
        return Err(eyre!("Reference value must not be empty"));
    }
    let asset = Asset {
        id: 0,
        name: name.to_string(),
        reference: args.reference.clone(),
    };
    let mut catgy_assignms: Vec<CategoryAssignment> = Vec::new();
    for (_, assignments) in args.category_id_to_assignment.iter() {
        let mut percentage = 0.;
        let mut seen_value_ids = HashSet::new();
        for assignment in assignments {
            if assignment.percentage == 0. {
                return Err(eyre!("Category value has percentage of 0%"));
            }
            if let Some(id) = assignment.value_id {
                if !seen_value_ids.insert(id) {
                    return Err(eyre!("Duplicate category values"));
                }
                percentage += assignment.percentage;
                catgy_assignms.push(CategoryAssignment {
                    value_id: id,
                    ratio: assignment.percentage / 100.,
                });
            } else {
                return Err(eyre!("Category value unset"));
            }
        }
        if percentage > 100. {
            return Err(eyre!("Percentages for a category add up to more than 100%"));
        }
    }
    add_asset_raw(&asset, &catgy_assignms)
}

fn backup_database(connection: &rusqlite::Connection, backup_path: &Path) -> eyre::Result<()> {
    connection.backup("main", backup_path, None)?;
    Ok(())
}

fn append_directory_to_tar(
    writer: &mut impl Write,
    source_dir: &Path,
    archive_path: &str,
) -> eyre::Result<()> {
    if !source_dir.exists() {
        append_tar_directory(writer, archive_path)?;
        return Ok(());
    }

    append_tar_directory(writer, archive_path)?;
    let mut entries = fs::read_dir(source_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let entry_name = entry.file_name();
        let entry_name = entry_name
            .to_str()
            .ok_or_else(|| eyre!("Backup path is not valid UTF-8"))?;
        let archive_entry_path = format!("{archive_path}/{entry_name}");

        if path.is_dir() {
            append_directory_to_tar(writer, &path, &archive_entry_path)?;
        } else if path.is_file() {
            append_file_to_tar(writer, &path, &archive_entry_path)?;
        }
    }

    Ok(())
}

fn append_file_to_tar(
    writer: &mut impl Write,
    source_path: &Path,
    archive_path: &str,
) -> eyre::Result<()> {
    let bytes = fs::read(source_path)?;
    write_tar_header(writer, archive_path, bytes.len() as u64, b'0')?;
    writer.write_all(&bytes)?;
    write_tar_padding(writer, bytes.len() as u64)?;
    Ok(())
}

fn append_tar_directory(writer: &mut impl Write, archive_path: &str) -> eyre::Result<()> {
    let archive_path = format!("{}/", archive_path.trim_end_matches('/'));
    write_tar_header(writer, &archive_path, 0, b'5')
}

fn write_tar_header(
    writer: &mut impl Write,
    archive_path: &str,
    size: u64,
    typeflag: u8,
) -> eyre::Result<()> {
    let mut header = [0_u8; 512];
    write_tar_string(&mut header[0..100], archive_path)?;
    write_tar_octal(&mut header[100..108], 0o644);
    write_tar_octal(&mut header[108..116], 0);
    write_tar_octal(&mut header[116..124], 0);
    write_tar_octal(&mut header[124..136], size);
    write_tar_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = typeflag;
    write_tar_string(&mut header[257..263], "ustar")?;
    write_tar_string(&mut header[263..265], "00")?;

    let checksum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    write_tar_checksum(&mut header[148..156], checksum);

    writer.write_all(&header)?;
    Ok(())
}

fn write_tar_string(field: &mut [u8], value: &str) -> eyre::Result<()> {
    let value = value.as_bytes();
    if value.len() > field.len() {
        return Err(eyre!("Backup archive path is too long"));
    }
    field[..value.len()].copy_from_slice(value);
    Ok(())
}

fn write_tar_octal(field: &mut [u8], value: u64) {
    let formatted = format!("{:0width$o}\0", value, width = field.len() - 1);
    field.copy_from_slice(formatted.as_bytes());
}

fn write_tar_checksum(field: &mut [u8], checksum: u32) {
    let formatted = format!("{:06o}\0 ", checksum);
    field.copy_from_slice(formatted.as_bytes());
}

fn write_tar_padding(writer: &mut impl Write, size: u64) -> eyre::Result<()> {
    let padding = (512 - (size % 512)) % 512;
    if padding > 0 {
        writer.write_all(&vec![0_u8; padding as usize])?;
    }
    Ok(())
}

fn write_tar_end(writer: &mut impl Write) -> eyre::Result<()> {
    writer.write_all(&[0_u8; 1024])?;
    Ok(())
}

pub fn log_buy_transaction(input: LogBuyTransactionInput) -> eyre::Result<()> {
    let transaction = validate_log_buy_transaction_input(input)?;
    let mut connection = open_transactions_connection()?;
    let tx = connection.transaction()?;
    let quantity = transaction.quantity.to_string();
    let transaction_id = insert_transaction(&tx, transaction)?;
    tx.execute(
        "
        INSERT INTO portfolio_items
            (buy_transaction_id, remaining_quantity)
        VALUES
            (?1, ?2)
        ",
        params![transaction_id, quantity],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn log_sell_transaction(input: LogSellTransactionInput) -> eyre::Result<()> {
    let sell_transaction = validate_log_sell_transaction_input(input)?;
    let mut connection = open_transactions_connection()?;
    let tx = connection.transaction()?;

    let asset_id = get_or_create_id(&tx, "assets", "isin", &sell_transaction.transaction.isin)?;
    for (portfolio_item_id, quantity) in &sell_transaction.portfolio_item_id_to_quantity {
        let (item_asset_id, remaining_quantity, buy_date, buy_time): (i64, String, String, String) =
            tx.query_row(
                "
            SELECT
                transactions.asset_id,
                portfolio_items.remaining_quantity,
                dates.date,
                transactions.time
            FROM portfolio_items
            JOIN transactions
                ON transactions.id = portfolio_items.buy_transaction_id
            JOIN dates
                ON dates.id = transactions.date_id
            WHERE portfolio_items.id = ?1
            ",
                params![portfolio_item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        if item_asset_id != asset_id {
            return Err(eyre!("Portfolio item does not belong to ISIN"));
        }
        let buy_date = jiff::civil::Date::strptime("%Y-%m-%d", &buy_date)?;
        let buy_time = parse_transaction_time("Buy time", &buy_time)?;
        if (
            sell_transaction.transaction.date,
            sell_transaction.transaction.time,
        ) < (buy_date, buy_time)
        {
            return Err(eyre!("Sell time must not be before buy time"));
        }
        let remaining_quantity = remaining_quantity
            .parse::<Decimal>()
            .map_err(|_| eyre!("Invalid remaining quantity for portfolio item"))?;
        if *quantity > remaining_quantity {
            return Err(eyre!("Sell quantity exceeds remaining quantity"));
        }
    }

    let sell_transaction_id = insert_transaction(&tx, sell_transaction.transaction)?;
    for (portfolio_item_id, quantity) in sell_transaction.portfolio_item_id_to_quantity {
        tx.execute(
            "
            INSERT INTO portfolio_item_sales
                (portfolio_item_id, sell_transaction_id, quantity)
            VALUES
                (?1, ?2, ?3)
            ",
            params![portfolio_item_id, sell_transaction_id, quantity.to_string()],
        )?;

        let remaining_quantity: String = tx.query_row(
            "SELECT remaining_quantity FROM portfolio_items WHERE id = ?1",
            params![portfolio_item_id],
            |row| row.get(0),
        )?;
        let remaining_quantity = remaining_quantity
            .parse::<Decimal>()
            .map_err(|_| eyre!("Invalid remaining quantity for portfolio item"))?;
        let new_remaining_quantity = remaining_quantity
            .checked_sub(quantity)
            .ok_or_else(|| eyre!("Remaining quantity is too small"))?;
        tx.execute(
            "
            UPDATE portfolio_items
            SET remaining_quantity = ?1
            WHERE id = ?2
            ",
            params![new_remaining_quantity.to_string(), portfolio_item_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn list_transactions() -> eyre::Result<Vec<ListedTransaction>> {
    let connection = open_transactions_connection()?;
    list_transactions_raw(&connection)
}

pub fn import_transaction_assets(
    input: ImportTransactionAssetsInput,
) -> eyre::Result<Vec<TransactionAsset>> {
    let mut connection = open_transactions_connection()?;

    let tx = connection.transaction()?;
    for raw_isin in parse_transaction_asset_isins(input.isins)? {
        let lookup = lookup_transaction_asset(&raw_isin)?;
        upsert_transaction_asset(&tx, lookup)?;
    }
    tx.commit()?;

    let connection = open_transactions_connection()?;
    list_transaction_assets_raw(&connection)
}

pub fn list_transaction_assets() -> eyre::Result<Vec<TransactionAsset>> {
    let connection = open_transactions_connection()?;
    list_transaction_assets_raw(&connection)
}

pub fn list_portfolio_overview_items() -> eyre::Result<Vec<PortfolioOverviewItem>> {
    let connection = open_transactions_connection()?;
    list_portfolio_overview_items_raw(&connection)
}

pub fn list_portfolio_isin_items(isin: String) -> eyre::Result<Vec<PortfolioIsinItem>> {
    let isin = normalize_isin(&isin)?;
    let connection = open_transactions_connection()?;
    list_portfolio_isin_items_raw(&connection, &isin)
}

#[derive(Debug)]
struct Transaction {
    r#type: TransactionType,
    currency: Currency,
    date: jiff::civil::Date,
    time: Time,
    isin: String,
    quantity: Decimal,
    share_price: Decimal,
    order_value: Decimal,
}

#[derive(Debug)]
struct SellTransaction {
    transaction: Transaction,
    portfolio_item_id_to_quantity: BTreeMap<i64, Decimal>,
}

fn validate_log_buy_transaction_input(input: LogBuyTransactionInput) -> eyre::Result<Transaction> {
    validate_transaction_date(input.date, input.client_today)?;
    let time = parse_transaction_time("Time", &input.time)?;
    let isin = normalize_isin(&input.isin)?;
    let quantity = parse_transaction_decimal("Quantity", &input.quantity)?;
    let share_price = parse_transaction_decimal("Share price", &input.share_price)?;
    let order_value = parse_transaction_decimal("Order value", &input.order_value)?;

    if quantity <= Decimal::ZERO {
        return Err(eyre!("Quantity must be greater than 0"));
    }
    if share_price <= Decimal::ZERO {
        return Err(eyre!("Share price must be greater than 0"));
    }
    let trade_value = quantity
        .checked_mul(share_price)
        .ok_or_else(|| eyre!("Quantity * share price is too large"))?;
    if order_value < trade_value {
        return Err(eyre!(
            "Order value must be greater than or equal to quantity * share price"
        ));
    }

    Ok(Transaction {
        r#type: TransactionType::Buy,
        currency: input.currency,
        date: input.date,
        time,
        isin,
        quantity,
        share_price,
        order_value,
    })
}

fn validate_log_sell_transaction_input(
    input: LogSellTransactionInput,
) -> eyre::Result<SellTransaction> {
    validate_transaction_date(input.date, input.client_today)?;
    let time = parse_transaction_time("Time", &input.time)?;
    let isin = normalize_isin(&input.isin)?;
    let share_price = parse_transaction_decimal("Share price", &input.share_price)?;
    let order_value = parse_transaction_decimal("Order value", &input.order_value)?;

    if share_price <= Decimal::ZERO {
        return Err(eyre!("Share price must be greater than 0"));
    }
    if order_value <= Decimal::ZERO {
        return Err(eyre!("Order value must be greater than 0"));
    }

    let mut total_quantity = Decimal::ZERO;
    let mut quantities = BTreeMap::new();
    for (portfolio_item_id, quantity_input) in input.portfolio_item_id_to_quantity {
        let quantity = parse_transaction_decimal("Quantity", &quantity_input)?;
        if quantity <= Decimal::ZERO {
            return Err(eyre!("Quantity must be greater than 0"));
        }
        total_quantity = total_quantity
            .checked_add(quantity)
            .ok_or_else(|| eyre!("Total quantity is too large"))?;
        quantities.insert(portfolio_item_id, quantity);
    }

    if quantities.is_empty() {
        return Err(eyre!("At least one sell quantity is required"));
    }

    let trade_value = total_quantity
        .checked_mul(share_price)
        .ok_or_else(|| eyre!("Quantity * share price is too large"))?;
    if order_value > trade_value {
        return Err(eyre!(
            "Order value must be less than or equal to quantity * share price"
        ));
    }

    Ok(SellTransaction {
        transaction: Transaction {
            r#type: TransactionType::Sell,
            currency: input.currency,
            date: input.date,
            time,
            isin,
            quantity: total_quantity,
            share_price,
            order_value,
        },
        portfolio_item_id_to_quantity: quantities,
    })
}

#[derive(Debug)]
struct TransactionAssetLookup {
    isin: String,
    symbol: Option<String>,
    name: Option<String>,
    exchange: Option<String>,
    quote_type: Option<String>,
    updated_at_date: String,
    updated_at_time: String,
}

fn parse_transaction_asset_isins(inputs: Vec<String>) -> eyre::Result<Vec<String>> {
    let mut isins = Vec::new();
    let mut seen = HashSet::new();

    for input in inputs {
        for token in input.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            let isin = normalize_isin(trimmed)?;
            if seen.insert(isin.clone()) {
                isins.push(isin);
            }
        }
    }

    if isins.is_empty() {
        return Err(eyre!("At least one ISIN is required"));
    }

    Ok(isins)
}

fn lookup_transaction_asset(isin: &str) -> eyre::Result<TransactionAssetLookup> {
    let mut search = rustyfinance::Search::new(isin);
    search.max_results = 1;
    search.news_count = 0;
    search.lists_count = 0;
    search.recommended = 0;
    search.fetch().map_err(|err| eyre!(err.to_string()))?;

    let quote = search.quotes().into_iter().next();
    let text_field = |key: &str| {
        quote
            .as_ref()
            .and_then(|quote| quote.get(key))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    let name = text_field("longname").or_else(|| text_field("shortname"));

    let now = jiff::Zoned::now();
    Ok(TransactionAssetLookup {
        isin: isin.to_string(),
        symbol: text_field("symbol"),
        name,
        exchange: text_field("exchDisp").or_else(|| text_field("exchange")),
        quote_type: text_field("quoteType"),
        updated_at_date: now.date().to_string(),
        updated_at_time: now.time().to_string(),
    })
}

fn parse_transaction_decimal(field_name: &str, input: &str) -> eyre::Result<Decimal> {
    input
        .trim()
        .parse::<Decimal>()
        .map_err(|_| eyre!("{field_name} must be a valid decimal number"))
}

fn parse_transaction_time(field_name: &str, input: &str) -> eyre::Result<Time> {
    Time::strptime("%H:%M:%S", input.trim())
        .map_err(|_| eyre!("{field_name} must be a valid time in HH:MM:SS format"))
}

fn validate_transaction_date(
    date: jiff::civil::Date,
    client_today: jiff::civil::Date,
) -> eyre::Result<()> {
    if date > client_today {
        return Err(eyre!("Transaction date must not be in the future"));
    }
    Ok(())
}

fn normalize_isin(input: &str) -> eyre::Result<String> {
    let isin = input.trim().to_ascii_uppercase();
    if !is_valid_isin(&isin) {
        return Err(eyre!("ISIN must be a valid 12-character ISIN"));
    }
    Ok(isin)
}

fn is_valid_isin(isin: &str) -> bool {
    let bytes = isin.as_bytes();
    if bytes.len() != 12 {
        return false;
    }
    if !bytes[0].is_ascii_uppercase() || !bytes[1].is_ascii_uppercase() {
        return false;
    }
    if !bytes[2..11].iter().all(u8::is_ascii_alphanumeric) || !bytes[11].is_ascii_digit() {
        return false;
    }
    let mut digits = Vec::with_capacity(24);
    for byte in bytes {
        if byte.is_ascii_digit() {
            digits.push(byte - b'0');
        } else if byte.is_ascii_uppercase() {
            let mut value = byte - b'A' + 10;
            digits.push(value / 10);
            value %= 10;
            digits.push(value);
        } else {
            return false;
        }
    }
    let mut sum = 0_u32;
    let mut double = false;
    for digit in digits.iter().rev() {
        let mut value = u32::from(*digit);
        if double {
            value *= 2;
            value = (value / 10) + (value % 10);
        }
        sum += value;
        double = !double;
    }
    sum.is_multiple_of(10)
}

fn initialize_transactions_schema(connection: &rusqlite::Connection) -> eyre::Result<()> {
    connection.execute_batch(&format!(
        "
        CREATE TABLE schema_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        INSERT INTO schema_metadata (key, value)
        VALUES ('{SCHEMA_VERSION_KEY}', '{TRANSACTIONS_DB_SCHEMA_VERSION}');

        CREATE TABLE assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            isin TEXT NOT NULL UNIQUE,
            symbol TEXT,
            name TEXT,
            exchange_id INTEGER,
            quote_type_id INTEGER,
            updated_at_date_id INTEGER,
            updated_at_time TEXT,
            FOREIGN KEY (exchange_id) REFERENCES exchanges(id),
            FOREIGN KEY (quote_type_id) REFERENCES quote_types(id),
            FOREIGN KEY (updated_at_date_id) REFERENCES dates(id)
        );

        CREATE TABLE currencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL UNIQUE
        );

        CREATE TABLE transaction_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL UNIQUE
        );

        CREATE TABLE dates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL UNIQUE
        );

        CREATE TABLE exchanges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE quote_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date_id INTEGER NOT NULL,
            time TEXT NOT NULL,
            type_id INTEGER NOT NULL,
            asset_id INTEGER NOT NULL,
            currency_id INTEGER NOT NULL,
            quantity TEXT NOT NULL,
            share_price TEXT NOT NULL,
            order_value TEXT NOT NULL,
            created_at_date_id INTEGER NOT NULL,
            created_at_time TEXT NOT NULL,
            FOREIGN KEY (date_id) REFERENCES dates(id),
            FOREIGN KEY (type_id) REFERENCES transaction_types(id),
            FOREIGN KEY (asset_id) REFERENCES assets(id),
            FOREIGN KEY (currency_id) REFERENCES currencies(id),
            FOREIGN KEY (created_at_date_id) REFERENCES dates(id)
        );

        CREATE TABLE portfolio_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            buy_transaction_id INTEGER NOT NULL UNIQUE,
            remaining_quantity TEXT NOT NULL,
            FOREIGN KEY (buy_transaction_id) REFERENCES transactions(id)
        );

        CREATE TABLE portfolio_item_sales (
            portfolio_item_id INTEGER NOT NULL,
            sell_transaction_id INTEGER NOT NULL,
            quantity TEXT NOT NULL,
            PRIMARY KEY (portfolio_item_id, sell_transaction_id),
            FOREIGN KEY (portfolio_item_id) REFERENCES portfolio_items(id),
            FOREIGN KEY (sell_transaction_id) REFERENCES transactions(id)
        );
        "
    ))?;
    Ok(())
}

fn initialize_assets_schema(connection: &rusqlite::Connection) -> eyre::Result<()> {
    connection.execute_batch(&format!(
        "
        CREATE TABLE schema_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        INSERT INTO schema_metadata (key, value)
        VALUES ('{SCHEMA_VERSION_KEY}', '{ASSETS_DB_SCHEMA_VERSION}');

        CREATE TABLE assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            reference_type TEXT NOT NULL,
            reference_value TEXT NOT NULL
        );

        CREATE TABLE asset_categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE asset_category_values (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            asset_category_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            UNIQUE(asset_category_id, name),
            FOREIGN KEY (asset_category_id) REFERENCES asset_categories(id)
        );

        CREATE TABLE asset_category_value_assignments (
            asset_id INTEGER NOT NULL,
            asset_category_value_id INTEGER NOT NULL,
            ratio REAL NOT NULL,
            PRIMARY KEY (asset_id, asset_category_value_id),
            FOREIGN KEY (asset_id) REFERENCES assets(id),
            FOREIGN KEY (asset_category_value_id) REFERENCES asset_category_values(id)
        );
        "
    ))?;
    Ok(())
}

fn ensure_database_schema_version(
    connection: &rusqlite::Connection,
    name: &str,
    current_version: u64,
) -> eyre::Result<()> {
    let stored_version = get_database_schema_version(connection, name)?;
    if stored_version != current_version {
        return Err(eyre!(
            "{name} schema version {stored_version} does not match supported version {current_version}"
        ));
    }
    Ok(())
}

fn get_database_schema_version(connection: &rusqlite::Connection, name: &str) -> eyre::Result<u64> {
    let has_metadata_table: bool = connection.query_row(
        "
        SELECT EXISTS (
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table'
                AND name = 'schema_metadata'
        )
        ",
        [],
        |row| row.get(0),
    )?;
    if !has_metadata_table {
        return Err(eyre!("{name} is missing schema version"));
    }

    let version = connection
        .query_row(
            "
            SELECT value
            FROM schema_metadata
            WHERE key = ?1
            ",
            [SCHEMA_VERSION_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| eyre!("{name} is missing schema version"))?;

    Ok(version.parse::<u64>()?)
}

fn get_or_create_id(
    connection: &rusqlite::Transaction<'_>,
    table_name: &str,
    column_name: &str,
    value: &str,
) -> eyre::Result<i64> {
    connection.execute(
        &format!("INSERT OR IGNORE INTO {table_name} ({column_name}) VALUES (?1)"),
        params![value],
    )?;
    let id = connection.query_row(
        &format!("SELECT id FROM {table_name} WHERE {column_name} = ?1"),
        params![value],
        |row| row.get(0),
    )?;
    Ok(id)
}

fn get_or_create_optional_id(
    connection: &rusqlite::Transaction<'_>,
    table_name: &str,
    column_name: &str,
    value: Option<&str>,
) -> eyre::Result<Option<i64>> {
    value
        .map(|value| get_or_create_id(connection, table_name, column_name, value))
        .transpose()
}

fn insert_transaction(
    connection: &rusqlite::Transaction<'_>,
    transaction: Transaction,
) -> eyre::Result<i64> {
    let asset_id = get_or_create_id(connection, "assets", "isin", &transaction.isin)?;
    let transaction_date = transaction.date.to_string();
    let date_id = get_or_create_id(connection, "dates", "date", &transaction_date)?;
    let transaction_time = transaction.time.strftime("%H:%M:%S").to_string();
    let now = jiff::Zoned::now();
    let created_at_date = now.date().to_string();
    let created_at_date_id = get_or_create_id(connection, "dates", "date", &created_at_date)?;
    let created_at_time = now.time().to_string();
    let currency_code = transaction.currency.to_string();
    let currency_id = get_or_create_id(connection, "currencies", "code", &currency_code)?;
    let transaction_type_str = transaction.r#type.to_string();
    let type_id = get_or_create_id(
        connection,
        "transaction_types",
        "code",
        &transaction_type_str,
    )?;

    connection.execute(
        "
        INSERT INTO transactions
            (
                date_id,
                time,
                type_id,
                asset_id,
                currency_id,
                quantity,
                share_price,
                order_value,
                created_at_date_id,
                created_at_time
            )
        VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ",
        params![
            date_id,
            transaction_time,
            type_id,
            asset_id,
            currency_id,
            transaction.quantity.to_string(),
            transaction.share_price.to_string(),
            transaction.order_value.to_string(),
            created_at_date_id,
            created_at_time,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

fn upsert_transaction_asset(
    connection: &rusqlite::Transaction<'_>,
    asset: TransactionAssetLookup,
) -> eyre::Result<()> {
    let exchange_id =
        get_or_create_optional_id(connection, "exchanges", "name", asset.exchange.as_deref())?;
    let quote_type_id = get_or_create_optional_id(
        connection,
        "quote_types",
        "name",
        asset.quote_type.as_deref(),
    )?;
    let updated_at_date_id = get_or_create_id(connection, "dates", "date", &asset.updated_at_date)?;

    connection.execute(
        "
        INSERT INTO assets
            (
                isin,
                symbol,
                name,
                exchange_id,
                quote_type_id,
                updated_at_date_id,
                updated_at_time
            )
        VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(isin) DO UPDATE SET
            symbol = excluded.symbol,
            name = excluded.name,
            exchange_id = excluded.exchange_id,
            quote_type_id = excluded.quote_type_id,
            updated_at_date_id = excluded.updated_at_date_id,
            updated_at_time = excluded.updated_at_time
        ",
        params![
            asset.isin,
            asset.symbol,
            asset.name,
            exchange_id,
            quote_type_id,
            updated_at_date_id,
            asset.updated_at_time,
        ],
    )?;
    Ok(())
}

fn list_transaction_assets_raw(
    connection: &rusqlite::Connection,
) -> eyre::Result<Vec<TransactionAsset>> {
    let mut statement = connection.prepare(
        "
        SELECT
            assets.id,
            assets.isin,
            assets.symbol,
            assets.name,
            exchanges.name,
            quote_types.name,
            dates.date,
            assets.updated_at_time
        FROM assets
        LEFT JOIN exchanges ON exchanges.id = assets.exchange_id
        LEFT JOIN quote_types ON quote_types.id = assets.quote_type_id
        LEFT JOIN dates ON dates.id = assets.updated_at_date_id
        ORDER BY COALESCE(assets.name, ''), assets.isin
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(TransactionAsset {
            id: row.get(0)?,
            isin: row.get(1)?,
            symbol: row.get(2)?,
            name: row.get(3)?,
            exchange: row.get(4)?,
            quote_type: row.get(5)?,
            updated_at_date: row.get(6)?,
            updated_at_time: row.get(7)?,
        })
    })?;

    let mut assets = Vec::new();
    for row in rows {
        assets.push(row?);
    }
    Ok(assets)
}

fn list_transactions_raw(
    connection: &rusqlite::Connection,
) -> eyre::Result<Vec<ListedTransaction>> {
    let mut statement = connection.prepare(
        "
        SELECT
            dates.date,
            transactions.time,
            transaction_types.code,
            assets.name,
            assets.isin,
            transactions.quantity,
            transactions.share_price,
            transactions.order_value,
            currencies.code
        FROM transactions
        JOIN dates ON dates.id = transactions.date_id
        JOIN transaction_types ON transaction_types.id = transactions.type_id
        JOIN assets ON assets.id = transactions.asset_id
        JOIN currencies ON currencies.id = transactions.currency_id
        ORDER BY dates.date DESC, transactions.time DESC, transactions.id DESC
        LIMIT 50
        ",
    )?;
    let transactions = statement
        .query_and_then([], |row| {
            let type_str: String = row.get(2)?;
            Ok(ListedTransaction {
                date: row.get(0)?,
                time: row.get(1)?,
                r#type: type_str
                    .parse()
                    .map_err(|_| eyre!("Invalid transaction type: {type_str}"))?,
                asset_name: row.get(3)?,
                isin: row.get(4)?,
                quantity: row.get(5)?,
                share_price: row.get(6)?,
                order_value: row.get(7)?,
                currency: row.get(8)?,
            })
        })?
        .collect::<eyre::Result<Vec<_>>>()?;
    Ok(transactions)
}

struct QueriedPortfolioItem {
    id: i64,
    buy_date: String,
    buy_time: String,
    asset_name: Option<String>,
    isin: String,
    quantity: String,
    share_price: String,
    order_value: String,
    currency: String,
}

fn query_portfolio_items(
    connection: &rusqlite::Connection,
    isin: Option<&str>,
) -> eyre::Result<Vec<QueriedPortfolioItem>> {
    let mut statement = connection.prepare(
        "
        SELECT
            portfolio_items.id,
            portfolio_items.buy_transaction_id,
            dates.date,
            transactions.time,
            assets.name,
            assets.isin,
            transactions.quantity,
            portfolio_items.remaining_quantity,
            transactions.share_price,
            transactions.order_value,
            currencies.code
        FROM portfolio_items
        JOIN transactions
            ON transactions.id = portfolio_items.buy_transaction_id
        JOIN dates
            ON dates.id = transactions.date_id
        JOIN assets
            ON assets.id = transactions.asset_id
        JOIN currencies
            ON currencies.id = transactions.currency_id
        WHERE (?1 IS NULL OR assets.isin = ?1)
        ORDER BY assets.isin ASC, dates.date ASC, transactions.time ASC, portfolio_items.id ASC
        ",
    )?;
    let items = statement
        .query_map(params![isin], |row| {
            Ok(QueriedPortfolioItem {
                id: row.get(0)?,
                buy_date: row.get(2)?,
                buy_time: row.get(3)?,
                asset_name: row.get(4)?,
                isin: row.get(5)?,
                quantity: row.get(7)?,
                share_price: row.get(8)?,
                order_value: row.get(9)?,
                currency: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|item| {
            item.quantity
                .parse::<Decimal>()
                .is_ok_and(|quantity| quantity > Decimal::ZERO)
        })
        .collect();
    Ok(items)
}

fn list_portfolio_isin_items_raw(
    connection: &rusqlite::Connection,
    isin: &str,
) -> eyre::Result<Vec<PortfolioIsinItem>> {
    Ok(query_portfolio_items(connection, Some(isin))?
        .into_iter()
        .map(|item| PortfolioIsinItem {
            portfolio_item_id: item.id,
            buy_date: item.buy_date,
            buy_time: item.buy_time,
            quantity: item.quantity,
            share_price: item.share_price,
            order_value: item.order_value,
            currency: item.currency,
        })
        .collect())
}

fn list_portfolio_overview_items_raw(
    connection: &rusqlite::Connection,
) -> eyre::Result<Vec<PortfolioOverviewItem>> {
    struct Accumulator {
        quantity: Decimal,
        total_value: Decimal,
    }

    let mut positions: BTreeMap<(Option<String>, String, String), Accumulator> = BTreeMap::new();
    for item in query_portfolio_items(connection, None)? {
        let quantity = item
            .quantity
            .parse::<Decimal>()
            .map_err(|_| eyre!("Invalid remaining quantity for portfolio item {}", item.id))?;
        let share_price = item
            .share_price
            .parse::<Decimal>()
            .map_err(|_| eyre!("Invalid share price for portfolio item {}", item.id))?;
        let item_value = quantity
            .checked_mul(share_price)
            .ok_or_else(|| eyre!("Portfolio item value is too large"))?;

        let position = positions
            .entry((item.asset_name, item.isin, item.currency))
            .or_insert(Accumulator {
                quantity: Decimal::ZERO,
                total_value: Decimal::ZERO,
            });
        position.quantity = position
            .quantity
            .checked_add(quantity)
            .ok_or_else(|| eyre!("Portfolio position quantity is too large"))?;
        position.total_value = position
            .total_value
            .checked_add(item_value)
            .ok_or_else(|| eyre!("Portfolio position total value is too large"))?;
    }

    positions
        .into_iter()
        .map(|((asset_name, isin, currency), position)| {
            let average_share_price = position
                .total_value
                .checked_div(position.quantity)
                .ok_or_else(|| eyre!("Portfolio position quantity must be greater than 0"))?;
            Ok(PortfolioOverviewItem {
                asset_name,
                isin,
                quantity: position.quantity.normalize().to_string(),
                average_share_price: average_share_price.normalize().to_string(),
                total_value: position.total_value.normalize().to_string(),
                currency,
            })
        })
        .collect()
}

fn add_asset_raw(asset: &Asset, catgy_assignms: &[CategoryAssignment]) -> eyre::Result<()> {
    let mut connection = open_assets_connection()?;
    let tx = connection.transaction()?;
    tx.execute(
        "INSERT INTO assets (name, reference_type, reference_value) VALUES (?1, ?2, ?3)",
        params![
            asset.name,
            asset.reference.r#type.to_string(),
            asset.reference.value
        ],
    )?;
    let asset_id = tx.last_insert_rowid();
    for assignm in catgy_assignms.iter() {
        tx.execute(
            "
            INSERT INTO asset_category_value_assignments
            (asset_id, asset_category_value_id, ratio)
            VALUES (?1, ?2, ?3)",
            params![asset_id, assignm.value_id, assignm.ratio],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_assets() -> eyre::Result<Vec<Asset>> {
    let connection = open_assets_connection()?;
    let mut stmt = connection.prepare(
        "SELECT id, name, reference_type, reference_value
                FROM assets
                ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let reference_type_str: String = row.get(2)?;
        let reference_type: AssetReferenceType = reference_type_str.parse().map_err(|_| {
            FromSqlError::Other(
                format!("Invalid AssetReferenceType: '{reference_type_str}'").into(),
            )
        })?;
        Ok(Asset {
            id: row.get(0)?,
            name: row.get(1)?,
            reference: AssetReference {
                r#type: reference_type,
                value: row.get(3)?,
            },
        })
    })?;
    let mut assets = Vec::new();
    for row in rows {
        assets.push(row?);
    }
    Ok(assets)
}

pub fn get_categories() -> eyre::Result<Vec<Category>> {
    let connection = open_assets_connection()?;

    let mut stmt = connection.prepare(
        "
        SELECT id, name
        FROM asset_categories
        ORDER BY name ASC
        ",
    )?;

    let category_rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut categories = Vec::new();

    for category_row in category_rows {
        let (category_id, category_name) = category_row?;

        let mut value_stmt = connection.prepare(
            "
            SELECT id, name
            FROM asset_category_values
            WHERE asset_category_id = ?
            ORDER BY name ASC
            ",
        )?;

        let values = value_stmt
            .query_map([category_id], |row| {
                Ok(CategoryValue {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        categories.push(Category {
            id: category_id,
            name: category_name,
            values,
        });
    }

    Ok(categories)
}

pub fn get_latest_record() -> eyre::Result<Option<AllocationRecord>> {
    Ok(get_latest_records(1)?.pop())
}

fn get_latest_record_paths(dir: &Path, limit: usize) -> eyre::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|ext| ext == "ron")
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| jiff::civil::Date::strptime("%Y-%m-%d", stem).is_ok())
        })
        .collect();

    paths.sort_by(|a, b| {
        let a = a.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let b = b.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        b.cmp(a) // newest first
    });

    paths.truncate(limit);
    Ok(paths)
}

fn get_latest_records(limit: usize) -> eyre::Result<Vec<AllocationRecord>> {
    ensure_data_dir()?;
    get_latest_record_paths(&allocation_records_dir(), limit)?
        .into_iter()
        .map(|path| read_allocation_record(&path))
        .collect()
}

fn read_allocation_record(path: &Path) -> eyre::Result<AllocationRecord> {
    let persisted: PersistedAllocationRecord = ron::from_str(&fs::read_to_string(path)?)?;
    if persisted.format_version != ALLOCATION_RECORD_FORMAT_VERSION {
        return Err(eyre!(
            "allocation record '{}' has format version {}, but supported version is {}",
            path.display(),
            persisted.format_version,
            ALLOCATION_RECORD_FORMAT_VERSION
        ));
    }
    Ok(persisted.record)
}

fn get_category_name_by_id(category_id: i64) -> eyre::Result<String> {
    let connection = open_assets_connection()?;
    Ok(connection.query_row(
        "SELECT name FROM asset_categories WHERE id = ?1",
        rusqlite::params![category_id],
        |row| row.get(0),
    )?)
}

fn add_category_value(category_id: i64, value_name: &str) -> eyre::Result<()> {
    let connection = open_assets_connection()?;
    connection.execute(
        "INSERT INTO asset_category_values (asset_category_id, name)
        VALUES (?1, ?2)",
        rusqlite::params![category_id, value_name],
    )?;
    Ok(())
}

fn add_category(name: &str) -> eyre::Result<i64> {
    let connection = open_assets_connection()?;
    connection.execute(
        "INSERT INTO asset_categories (name) VALUES (?1)",
        params![name],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn configure_categories(
    input: ConfigureCatgoriesInput,
) -> eyre::Result<(ConfigureCatgoriesInput, Option<String>)> {
    let mut remaining = ConfigureCatgoriesInput::default();
    let mut first_error: Option<String> = None;

    // Neue Kategorien + deren neue Values
    for new_category in input.new_category_inputs {
        let category_name = new_category.name.trim();

        if category_name.is_empty() {
            remaining.new_category_inputs.push(new_category);
            continue;
        }

        match add_category(category_name) {
            Ok(category_id) => {
                let mut remaining_values = Vec::new();

                for value_input in new_category.new_value_inputs {
                    let value_name = value_input.name.trim();

                    if value_name.is_empty() {
                        remaining_values.push(value_input);
                        continue;
                    }

                    if let Err(err) = add_category_value(category_id, value_name) {
                        if first_error.is_none() {
                            first_error = Some(err.to_string());
                        }
                        remaining_values.push(value_input);
                    }
                }

                if !remaining_values.is_empty() {
                    remaining.new_category_inputs.push(NewCategoryInput {
                        name: new_category.name,
                        new_value_inputs: remaining_values,
                    });
                }
            }
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(err.to_string());
                }

                remaining.new_category_inputs.push(new_category);
            }
        }
    }

    // Bestehende Kategorien erweitern
    for (category_id, adapt_input) in input.category_id_to_adapt_input {
        let mut remaining_values = Vec::new();

        for value_input in adapt_input.new_value_inputs {
            let value_name = value_input.name.trim();

            if value_name.is_empty() {
                remaining_values.push(value_input);
                continue;
            }

            if let Err(err) = add_category_value(category_id, value_name) {
                if first_error.is_none() {
                    first_error = Some(err.to_string());
                }
                remaining_values.push(value_input);
            }
        }

        if !remaining_values.is_empty() {
            remaining.category_id_to_adapt_input.insert(
                category_id,
                AdaptCategoryInput {
                    new_value_inputs: remaining_values,
                },
            );
        }
    }

    Ok((remaining, first_error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use jiff::Zoned;
    use std::{
        collections::HashMap,
        io::Read,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn valid_log_buy_transaction_input() -> LogBuyTransactionInput {
        LogBuyTransactionInput {
            currency: Currency::Eur,
            date: Zoned::now().date(),
            time: "12:34:56".to_string(),
            client_today: Zoned::now().date(),
            isin: "US0378331005".to_string(),
            quantity: "2.5".to_string(),
            share_price: "100.00".to_string(),
            order_value: "250.00".to_string(),
        }
    }

    fn valid_log_sell_transaction_input() -> LogSellTransactionInput {
        LogSellTransactionInput {
            currency: Currency::Eur,
            date: Zoned::now().date(),
            time: "12:34:56".to_string(),
            client_today: Zoned::now().date(),
            isin: "US0378331005".to_string(),
            portfolio_item_id_to_quantity: HashMap::from([(1, "1.5".to_string())]),
            share_price: "100.00".to_string(),
            order_value: "149.99".to_string(),
        }
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("tallytail_{name}_{unique}"))
    }

    #[test]
    fn creates_assets_schema_for_empty_database() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();

        initialize_assets_schema(&connection).unwrap();
        ensure_database_schema_version(&connection, "assets database", ASSETS_DB_SCHEMA_VERSION)
            .unwrap();

        connection
            .execute(
                "INSERT INTO asset_categories (name) VALUES (?1)",
                ["Sector"],
            )
            .unwrap();
        let category_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO asset_category_values (asset_category_id, name) VALUES (?1, ?2)",
                rusqlite::params![category_id, "Technology"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO assets (name, reference_type, reference_value) VALUES (?1, ?2, ?3)",
                ["Apple", "Isin", "US0378331005"],
            )
            .unwrap();

        let asset_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .unwrap();
        let category_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM asset_categories", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(asset_count, 1);
        assert_eq!(category_count, 1);
    }

    #[test]
    fn creates_schema_version_for_empty_assets_database() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();

        initialize_assets_schema(&connection).unwrap();

        let version = get_database_schema_version(&connection, "assets database").unwrap();
        assert_eq!(version, ASSETS_DB_SCHEMA_VERSION);
    }

    #[test]
    fn creates_transactions_schema_with_execution_time() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();

        initialize_transactions_schema(&connection).unwrap();

        let has_time_column: bool = connection
            .prepare("PRAGMA table_info(transactions)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .any(|column| column.unwrap() == "time");
        assert!(has_time_column);
    }

    #[test]
    fn rejects_existing_assets_database_without_schema_version() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute(
                "CREATE TABLE assets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    reference_type TEXT NOT NULL,
                    reference_value TEXT NOT NULL
                )",
                [],
            )
            .unwrap();

        let err = ensure_database_schema_version(
            &connection,
            "assets database",
            ASSETS_DB_SCHEMA_VERSION,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("missing schema version"));
    }

    #[test]
    fn rejects_allocation_record_with_unsupported_format_version() {
        let dir = unique_test_path("allocation_records");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-07-06.ron");
        let persisted = PersistedAllocationRecord {
            format_version: ALLOCATION_RECORD_FORMAT_VERSION + 1,
            record: AllocationRecord {
                date: "2026-07-06".to_string(),
                positions: Vec::new(),
            },
        };
        std::fs::write(&path, ron::to_string(&persisted).unwrap()).unwrap();

        let err = read_allocation_record(&path).unwrap_err().to_string();

        assert!(err.contains("format version"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn latest_record_paths_creates_missing_directory() {
        let dir = unique_test_path("allocation_records");

        let paths = get_latest_record_paths(&dir, 1).unwrap();

        assert!(paths.is_empty());
        assert!(dir.is_dir());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_data_backup_with_only_restore_root_entries() {
        let dir = unique_test_path("data_backup");
        let previous_data_dir = env::var_os("TALLYTAIL_DATA_DIR");
        unsafe {
            env::set_var("TALLYTAIL_DATA_DIR", &dir);
        }
        fs::create_dir_all(dir.join("allocation_records")).unwrap();
        fs::write(dir.join("allocation_records").join("record.ron"), "record").unwrap();

        let archive = create_data_backup().unwrap();
        let entry_names = tar_entry_names_from_gzip(&archive);

        assert!(entry_names.contains(&"assets.sdb".to_string()));
        assert!(entry_names.contains(&"transactions.sdb".to_string()));
        assert!(entry_names.contains(&"allocation_records/".to_string()));
        assert!(entry_names.contains(&"allocation_records/record.ron".to_string()));
        assert!(!entry_names.iter().any(|name| name.starts_with("data/")));

        std::fs::remove_dir_all(dir).unwrap();
        unsafe {
            if let Some(previous_data_dir) = previous_data_dir {
                env::set_var("TALLYTAIL_DATA_DIR", previous_data_dir);
            } else {
                env::remove_var("TALLYTAIL_DATA_DIR");
            }
        }
    }

    #[test]
    fn accepts_valid_isin_with_check_digit() {
        assert!(is_valid_isin("US0378331005"));
    }

    #[test]
    fn rejects_invalid_isin_check_digit() {
        assert!(!is_valid_isin("US0378331006"));
    }

    #[test]
    fn parses_transaction_asset_isin_list() {
        let isins = parse_transaction_asset_isins(vec![
            "us0378331005\nUS5949181045".to_string(),
            "US0378331005; US0231351067".to_string(),
        ])
        .unwrap();

        assert_eq!(isins, vec!["US0378331005", "US5949181045", "US0231351067"]);
    }

    #[test]
    fn rejects_empty_transaction_asset_isin_list() {
        let err = parse_transaction_asset_isins(vec![" \n , ; ".to_string()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("At least one ISIN"));
    }

    #[test]
    fn accepts_transaction_date_on_client_today() {
        let mut input = valid_log_buy_transaction_input();
        input.client_today = input.date;

        validate_log_buy_transaction_input(input).unwrap();
    }

    #[test]
    fn rejects_future_transaction_date() {
        let mut input = valid_log_buy_transaction_input();
        input.date = input.client_today.tomorrow().unwrap();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("future"));
    }

    #[test]
    fn rejects_buy_order_value_below_quantity_times_share_price() {
        let mut input = valid_log_buy_transaction_input();
        input.order_value = "249.99".to_string();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("quantity * share price"));
    }

    #[test]
    fn rejects_quantity_less_than_or_equal_to_zero() {
        for quantity in ["0", "-1"] {
            let mut input = valid_log_buy_transaction_input();
            input.quantity = quantity.to_string();

            let err = validate_log_buy_transaction_input(input)
                .unwrap_err()
                .to_string();

            assert!(err.contains("Quantity must be greater than 0"));
        }
    }

    #[test]
    fn rejects_share_price_less_than_or_equal_to_zero() {
        for share_price in ["0", "-1"] {
            let mut input = valid_log_buy_transaction_input();
            input.share_price = share_price.to_string();

            let err = validate_log_buy_transaction_input(input)
                .unwrap_err()
                .to_string();

            assert!(err.contains("Share price must be greater than 0"));
        }
    }

    #[test]
    fn rejects_invalid_quantity_decimal_format() {
        let mut input = valid_log_buy_transaction_input();
        input.quantity = "not-a-decimal".to_string();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Quantity must be a valid decimal number"));
    }

    #[test]
    fn rejects_invalid_share_price_decimal_format() {
        let mut input = valid_log_buy_transaction_input();
        input.share_price = "not-a-decimal".to_string();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Share price must be a valid decimal number"));
    }

    #[test]
    fn rejects_invalid_order_value_decimal_format() {
        let mut input = valid_log_buy_transaction_input();
        input.order_value = "not-a-decimal".to_string();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Order value must be a valid decimal number"));
    }

    #[test]
    fn rejects_invalid_buy_transaction_time_format() {
        let mut input = valid_log_buy_transaction_input();
        input.time = "12:34".to_string();

        let err = validate_log_buy_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("HH:MM:SS"));
    }

    #[test]
    fn accepts_valid_sell_transaction_input() {
        validate_log_sell_transaction_input(valid_log_sell_transaction_input()).unwrap();
    }

    #[test]
    fn rejects_sell_transaction_without_quantities() {
        let mut input = valid_log_sell_transaction_input();
        input.portfolio_item_id_to_quantity.clear();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("At least one sell quantity"));
    }

    #[test]
    fn rejects_sell_transaction_future_date() {
        let mut input = valid_log_sell_transaction_input();
        input.date = input.client_today.tomorrow().unwrap();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("future"));
    }

    #[test]
    fn rejects_sell_transaction_invalid_quantity_decimal_format() {
        let mut input = valid_log_sell_transaction_input();
        input
            .portfolio_item_id_to_quantity
            .insert(1, "not-a-decimal".to_string());

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Quantity must be a valid decimal number"));
    }

    #[test]
    fn rejects_sell_transaction_quantity_less_than_or_equal_to_zero() {
        for quantity in ["0", "-1"] {
            let mut input = valid_log_sell_transaction_input();
            input
                .portfolio_item_id_to_quantity
                .insert(1, quantity.to_string());

            let err = validate_log_sell_transaction_input(input)
                .unwrap_err()
                .to_string();

            assert!(err.contains("Quantity must be greater than 0"));
        }
    }

    #[test]
    fn rejects_sell_transaction_share_price_less_than_or_equal_to_zero() {
        for share_price in ["0", "-1"] {
            let mut input = valid_log_sell_transaction_input();
            input.share_price = share_price.to_string();

            let err = validate_log_sell_transaction_input(input)
                .unwrap_err()
                .to_string();

            assert!(err.contains("Share price must be greater than 0"));
        }
    }

    #[test]
    fn rejects_sell_transaction_order_value_less_than_or_equal_to_zero() {
        for order_value in ["0", "-1"] {
            let mut input = valid_log_sell_transaction_input();
            input.order_value = order_value.to_string();

            let err = validate_log_sell_transaction_input(input)
                .unwrap_err()
                .to_string();

            assert!(err.contains("Order value must be greater than 0"));
        }
    }

    #[test]
    fn rejects_sell_transaction_invalid_share_price_decimal_format() {
        let mut input = valid_log_sell_transaction_input();
        input.share_price = "not-a-decimal".to_string();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Share price must be a valid decimal number"));
    }

    #[test]
    fn rejects_sell_transaction_invalid_order_value_decimal_format() {
        let mut input = valid_log_sell_transaction_input();
        input.order_value = "not-a-decimal".to_string();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("Order value must be a valid decimal number"));
    }

    #[test]
    fn rejects_invalid_sell_transaction_time_format() {
        let mut input = valid_log_sell_transaction_input();
        input.time = "12:34".to_string();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("HH:MM:SS"));
    }

    #[test]
    fn rejects_sell_transaction_order_value_above_quantity_times_share_price() {
        let mut input = valid_log_sell_transaction_input();
        input.order_value = "150.01".to_string();

        let err = validate_log_sell_transaction_input(input)
            .unwrap_err()
            .to_string();

        assert!(err.contains("quantity * share price"));
    }

    fn tar_entry_names_from_gzip(archive: &[u8]) -> Vec<String> {
        let mut tar_bytes = Vec::new();
        GzDecoder::new(archive).read_to_end(&mut tar_bytes).unwrap();

        let mut names = Vec::new();
        let mut offset = 0;
        while offset + 512 <= tar_bytes.len() {
            let header = &tar_bytes[offset..offset + 512];
            if header.iter().all(|byte| *byte == 0) {
                break;
            }

            let name_end = header[0..100]
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(100);
            names.push(String::from_utf8(header[0..name_end].to_vec()).unwrap());

            let size_end = header[124..136]
                .iter()
                .position(|byte| *byte == 0 || *byte == b' ')
                .unwrap_or(12);
            let size = u64::from_str_radix(
                std::str::from_utf8(&header[124..124 + size_end])
                    .unwrap()
                    .trim(),
                8,
            )
            .unwrap();
            let padded_size = size.div_ceil(512) * 512;
            offset += 512 + padded_size as usize;
        }

        names
    }
}
