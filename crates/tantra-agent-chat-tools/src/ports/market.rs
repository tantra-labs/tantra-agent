//! Market read port and DTO contracts (§6.4).

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::{InstrumentId, LevelMap, Money, Price, PriceBand, Quantity};

use crate::ports::error::PortError;
use crate::ports::portfolio::DateRange;

/// Latest market quote for an instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketQuote {
    /// Instrument identifier.
    pub instrument: InstrumentId,
    /// Last traded price.
    pub last_price: Price,
    /// Best bid price.
    pub bid: Option<Price>,
    /// Best ask price.
    pub ask: Option<Price>,
    /// Last traded or cumulative volume.
    pub volume: Quantity,
    /// Quote observation timestamp.
    pub as_of: DateTime<Utc>,
}

/// OHLCV bar for market history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketBar {
    /// Bar start timestamp.
    pub timestamp: DateTime<Utc>,
    /// Open price.
    pub open: Price,
    /// High price.
    pub high: Price,
    /// Low price.
    pub low: Price,
    /// Close price.
    pub close: Price,
    /// Bar volume.
    pub volume: Quantity,
}

/// Historical series of bars with downsampling marker (§6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketHistory {
    /// Instrument identifier.
    pub instrument: InstrumentId,
    /// Bars in chronological order.
    pub bars: Vec<MarketBar>,
    /// True if data was downsampled or truncated to cap limit (§6.5).
    pub downsampled: bool,
}

/// Set of technical indicators computed for an instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketIndicators {
    /// Instrument identifier.
    pub instrument: InstrumentId,
    /// Computed indicator values (e.g. "rsi_14" -> 62.5, "sma_20" -> 2450.0).
    pub values: HashMap<String, Decimal>,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
}

/// Search hit when discovering instruments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSearchResult {
    /// Canonical instrument identifier.
    pub instrument: InstrumentId,
    /// Human-readable scrip/security name.
    pub name: String,
    /// Trading venue / exchange.
    pub exchange: String,
    /// Market lot size if applicable.
    pub lot_size: Option<u64>,
}

/// Macro-economic series data points (§6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroSeries {
    /// Canonical series identifier (e.g. "cpi", "rbi_repo_rate", "iip").
    pub series: String,
    /// Time-stamped metric values.
    pub points: Vec<(DateTime<Utc>, Decimal)>,
}

/// A single disclosed block or bulk deal (exchange disclosure feed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDeal {
    /// Instrument identifier.
    pub instrument: InstrumentId,
    /// Exchange that disclosed the deal (e.g. "NSE", "BSE").
    pub exchange: String,
    /// Disclosed client / trading-member name.
    pub client_name: String,
    /// Deal side: "buy" or "sell".
    pub side: String,
    /// Deal category: "block" or "bulk".
    pub category: String,
    /// Traded quantity.
    pub quantity: Quantity,
    /// Traded price.
    pub price: Price,
    /// Disclosure / trade date.
    pub trade_date: DateTime<Utc>,
}

/// One listed option contract in a chain snapshot (plan 0006 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionContract {
    /// The option contract itself (e.g. "NFO:NIFTY26NOV24000CE").
    pub instrument: InstrumentId,
    /// Option type: "CE" (call) or "PE" (put) — the canonical FNO symbol-grammar
    /// codes (`tantra-market-finance`'s `OptionType`).
    pub option_type: String,
    /// Strike price.
    pub strike: Price,
    /// Contract expiry (midnight UTC when only a calendar date exists).
    pub expiry: DateTime<Utc>,
    /// Trading lot size.
    pub lot_size: Quantity,
    /// Last traded price, when quoted.
    pub last_price: Option<Price>,
    /// Best bid price, when quoted.
    pub bid: Option<Price>,
    /// Best ask price, when quoted.
    pub ask: Option<Price>,
    /// Open interest, when reported.
    pub open_interest: Option<Quantity>,
}

/// Options-chain snapshot for one underlying instrument (plan 0006 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionsChain {
    /// Underlying instrument identifier.
    pub underlying: InstrumentId,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
    /// Contracts, ordered by expiry, then strike, then option type.
    pub contracts: Vec<OptionContract>,
}

/// One realized corporate action (plan 0006 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateAction {
    /// Instrument the action applies to.
    pub instrument: InstrumentId,
    /// Canonical action category: "dividend", "split", "bonus", or "rights"
    /// (mirrors `tantra_data.domain.corporate_actions`' value objects).
    pub action_type: String,
    /// Ex-date — the date the instrument trades at the post-action price.
    pub ex_date: DateTime<Utc>,
    /// Record date, when disclosed.
    pub record_date: Option<DateTime<Utc>>,
    /// Per-share dividend amount (dividend actions).
    pub amount: Option<Money>,
    /// Ratio "from" side for split/bonus/rights actions (shares held before),
    /// e.g. a 1:2 split carries `ratio_from = 1`, `ratio_to = 2`.
    pub ratio_from: Option<Quantity>,
    /// Ratio "to" side for split/bonus/rights actions.
    pub ratio_to: Option<Quantity>,
}

/// One level of the order-book depth ladder (plan 0007 Track 2 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketDepthLevel {
    /// Price of this book level.
    pub price: Price,
    /// Quantity resting at this level.
    pub quantity: Quantity,
}

/// Level-2 order-book depth snapshot with liquidity imbalance (plan 0007 Track 2 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketDepth {
    /// Instrument the depth belongs to.
    pub instrument: InstrumentId,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
    /// Bid levels, best first (highest price first).
    pub bids: Vec<MarketDepthLevel>,
    /// Ask levels, best first (lowest price first).
    pub asks: Vec<MarketDepthLevel>,
    /// Liquidity imbalance percent = (bid_qty - ask_qty)/(bid_qty + ask_qty) * 100.
    pub liquidity_imbalance_pct: Decimal,
}

/// One day's FII or DII net flow for the cash segment (plan 0007 Track 2 Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstitutionalFlow {
    /// "fii" or "dii".
    pub category: String,
    /// Gross buy value for the day.
    pub buy_value: Money,
    /// Gross sell value for the day.
    pub sell_value: Money,
    /// Net (buy minus sell) value for the day.
    pub net_value: Money,
    /// Trade date the flow belongs to.
    pub trade_date: DateTime<Utc>,
}

/// One day's delivery-adjusted volume observation (plan 0010 Phase 2).
///
/// `delivery_pct` is the exact `delivered_qty / traded_qty * 100` ratio — computed and
/// stored as `Decimal`, never a float ratio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryBar {
    /// Instrument the delivery observation belongs to.
    pub instrument: InstrumentId,
    /// Trade date the observation was taken for.
    pub date: DateTime<Utc>,
    /// Total quantity traded that day.
    pub traded_qty: Quantity,
    /// Quantity delivered into demat that day.
    pub delivered_qty: Quantity,
    /// Delivery percentage = delivered / traded * 100, as an exact decimal.
    pub delivery_pct: Decimal,
}

/// One instrument's open-interest buildup classification over a window (plan 0010 Phase 3).
///
/// `classification` is one of `"long_buildup"` (price↑, OI↑) | `"short_buildup"`
/// (price↓, OI↑) | `"short_covering"` (price↑, OI↓) | `"long_unwinding"` (price↓, OI↓).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OiBuildupSnapshot {
    /// The instrument whose position was analysed.
    pub instrument: InstrumentId,
    /// End-of-window snapshot timestamp.
    pub as_of: DateTime<Utc>,
    /// Open interest at the end of the window.
    pub open_interest: Quantity,
    /// Open-interest change over the window as a percent of the window-start value.
    pub oi_change_pct: Option<Decimal>,
    /// Close-to-close price change over the window as a percent of the window-start close.
    pub price_change_pct: Option<Decimal>,
    /// The four-quadrant class: `long_buildup` | `short_buildup` | `short_covering` | `long_unwinding`.
    pub classification: String,
}

/// One open-interest wall in a chain (plan 0011 / EE+ Layer A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OiWall {
    /// Strike price of the wall.
    pub strike: Price,
    /// Open interest resting at the strike.
    pub oi: Decimal,
    /// Delta-open-interest building score (exact decimal, higher = building faster).
    pub delta_oi_score: Decimal,
}

/// One instrument/expiry's open-interest wall snapshot (plan 0011 / EE+ Layer A-C inputs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OiWallsSnapshot {
    /// Top call-side OI walls by open interest.
    pub top_call_walls: Vec<OiWall>,
    /// Top put-side OI walls by open interest.
    pub top_put_walls: Vec<OiWall>,
    /// Local put-call ratio for the chain.
    pub local_pcr: Decimal,
    /// Snapshot timestamp.
    pub as_of: DateTime<Utc>,
}

/// Read-only market data port (§6.4).
///
/// Implemented by runtime application layer over market feeds and history stores.
#[async_trait]
pub trait MarketReader: Send + Sync + 'static {
    /// Fetch latest quote for an instrument.
    async fn quote(&self, instrument: &InstrumentId) -> Result<MarketQuote, PortError>;

    /// Fetch historical bars capped at 500 bars (downsampled if larger) (§6.5).
    async fn history(
        &self,
        instrument: &InstrumentId,
        range: DateRange,
        limit: Option<usize>,
    ) -> Result<MarketHistory, PortError>;

    /// Fetch requested technical indicators for an instrument.
    async fn indicators(
        &self,
        instrument: &InstrumentId,
        names: &[String],
    ) -> Result<MarketIndicators, PortError>;

    /// Search for instruments by query string (symbol or name).
    async fn search_instruments(
        &self,
        query: &str,
    ) -> Result<Vec<InstrumentSearchResult>, PortError>;

    /// Fetch macro-economic series data (e.g. CPI, IIP, RBI repo rate).
    async fn macro_series(&self, series: &str) -> Result<MacroSeries, PortError>;

    /// Fetch disclosed block/bulk deals for a date (defaults to latest trading day) or instrument.
    async fn block_deals(
        &self,
        instrument: Option<&InstrumentId>,
        date: Option<DateTime<Utc>>,
    ) -> Result<Vec<BlockDeal>, PortError>;

    /// Fetch pre-trade price bands for an instrument.
    async fn price_band(&self, instrument: &InstrumentId) -> Result<PriceBand, PortError>;

    /// Fetch the listed option chain for an underlying, optionally filtered by expiry
    /// date, capped at `limit` (implementor-chosen default when `None`).
    ///
    /// Upstream state (verified 2026-11-12): `tantra-market` holds contract-reference
    /// and Greeks-pricing building blocks but no chain enumeration/reader — see
    /// `tantra-market/docs/gaps/0001-options-chain-reader.md`. Implementors without a
    /// chain source must answer `PortError::unavailable` citing that gap — never an
    /// empty success.
    async fn options_chain(
        &self,
        underlying: &InstrumentId,
        expiry: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<OptionsChain, PortError>;

    /// Fetch realized corporate actions (dividend/split/bonus/rights), optionally
    /// filtered by instrument and action type, capped at `limit`.
    ///
    /// Upstream state (verified 2026-11-12): the historical corporate-actions reader
    /// exists Python-side (`tantra_data`'s `CorporateActionsRepository`/
    /// `DuckDbCorporateActionsStore`) but has no Rust-consumer bridge — see
    /// `tantra-data/docs/gaps/0008-corporate-actions-rust-bridge.md`. Implementors
    /// without a bridge must answer `PortError::unavailable` citing that gap.
    async fn corporate_actions(
        &self,
        instrument: Option<&InstrumentId>,
        action_type: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<CorporateAction>, PortError>;

    /// Level-2 order-book depth (best `levels` bid/ask levels, default 5).
    ///
    /// Upstream state: no L2 depth source — `tantra-market/docs/gaps/0006-l2-order-book-depth.md`.
    /// Implementors without a depth source must answer `PortError::unavailable` citing that gap.
    async fn market_depth(
        &self,
        instrument: &InstrumentId,
        levels: Option<usize>,
    ) -> Result<MarketDepth, PortError>;

    /// Daily FII/DII institutional net flows for the cash segment.
    ///
    /// Upstream state: the `tantra-data` → Rust bridge exists
    /// (`InstitutionalFlowsProjectionReader`, `FeedMarketReader::with_institutional_flows` —
    /// `tantra-data` gap 0017 closed). Implementors without a projection reader wired must
    /// answer `PortError::unavailable` citing `tantra-runtime/docs/gaps/0007` item 11.
    async fn institutional_flows(
        &self,
        date: Option<DateTime<Utc>>,
    ) -> Result<Vec<InstitutionalFlow>, PortError>;

    /// Delivery-adjusted volume history for an instrument, capped at `limit`.
    ///
    /// Upstream state: the `tantra-data` → Rust bridge exists
    /// (`DeliveryProjectionReader`, `FeedMarketReader::with_delivery` — `tantra-data` gap 0019
    /// closed). Implementors without a projection reader wired must answer
    /// `PortError::unavailable` citing `tantra-runtime/docs/gaps/0007` item 12.
    async fn delivery_stats(
        &self,
        instrument: &InstrumentId,
        range: DateRange,
        limit: Option<usize>,
    ) -> Result<Vec<DeliveryBar>, PortError>;

    /// Open-interest buildup classification for an instrument over a lookback window.
    ///
    /// Upstream state: the OI time-series store + four-quadrant classifier live in
    /// `tantra-market-store` (`tantra-market/docs/gaps/0002` closed), but the runtime chat
    /// adapters have no OI store wired — `tantra-runtime/docs/gaps/0007` item 12.
    /// Implementors without an OI store must answer `PortError::unavailable`.
    async fn oi_buildup(
        &self,
        instrument: &InstrumentId,
        lookback_days: Option<u32>,
    ) -> Result<OiBuildupSnapshot, PortError>;

    /// Fetch the session's intraday level map for an instrument — Elephant percentile
    /// zones, OI walls, gamma flip, max pain, PDH/PDL, and order blocks, each with a
    /// confluence score — serialized from `tantra_core::LevelMap` (plan 0011 / EE+).
    ///
    /// Upstream state (verified 2026-11-17): the kernel wire types exist (`tantra-core`
    /// `levels`, plan 0003), but no source serves the map — the Elephant-zone / OI-wall /
    /// gamma / order-block indicators are an open gap
    /// (`tantra-market/docs/gaps/0005-elephant-zones-oi-levels-and-orderblock-indicators.md`)
    /// and no as-of level-map snapshot store exists
    /// (`tantra-data/docs/gaps/0016-intraday-option-chain-snapshot-lakehouse.md`).
    /// Implementors without a level-map source must answer `PortError::unavailable`
    /// citing those gaps — never an empty map.
    async fn level_map(
        &self,
        instrument: &InstrumentId,
        date: Option<NaiveDate>,
    ) -> Result<LevelMap, PortError>;

    /// Fetch the top call/put open-interest walls with delta-OI building scores and the
    /// local put-call ratio for an instrument/expiry (plan 0011 / EE+ Layer A-C inputs).
    ///
    /// Upstream state (verified 2026-11-17): the intraday option-chain snapshot lakehouse
    /// (OI/ΔOI/IV/delta/gamma point-in-time series) does not exist — see
    /// `tantra-data/docs/gaps/0016-intraday-option-chain-snapshot-lakehouse.md` — and the
    /// per-strike indicator feeds remain an open `tantra-market` gap
    /// (`tantra-market/docs/gaps/0005-elephant-zones-oi-levels-and-orderblock-indicators.md`).
    /// Implementors without a chain-snapshot source must answer `PortError::unavailable`
    /// citing those gaps — never an empty success.
    async fn oi_walls(
        &self,
        instrument: &InstrumentId,
        expiry: Option<NaiveDate>,
    ) -> Result<OiWallsSnapshot, PortError>;
}
