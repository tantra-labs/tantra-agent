//! Market read port and DTO contracts (§6.4).

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::{InstrumentId, Money, Price, PriceBand, Quantity};

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
    /// Upstream state: data exists Python-side (`tantra-data` gap 0013 closed) but no Rust
    /// bridge — `tantra-data/docs/gaps/0017-institutional-flow-rust-bridge.md`.
    async fn institutional_flows(
        &self,
        date: Option<DateTime<Utc>>,
    ) -> Result<Vec<InstitutionalFlow>, PortError>;
}
