//! Domain read ports owned by `tantra-agent-chat-tools` (§6.2, §6.4).
//!
//! Enforces compile-time read-only safety: chat tools hold references to these
//! traits and cannot access mutators or execution systems.

pub mod alert;
pub mod calendar;
pub mod content;
pub mod error;
pub mod fund;
pub mod fundamental;
pub mod ipo;
pub mod market;
pub mod portfolio;
pub mod snapshot;
pub mod strategy_store;
pub mod tax;
pub mod trade_intent;

pub use alert::{AlertReader, AlertStatus, AlertWriter, PriceAlert};
pub use calendar::{CalendarReader, EconomicCalendarQuery, EconomicEvent};
pub use content::{ContentReader, FilingRecord, NewsArticle};
pub use error::{PortError, PortErrorKind};
pub use fund::{FundHolding, FundReader, MutualFundMeta};
pub use fundamental::{FinancialStatement, FundamentalRatios, FundamentalReader};
pub use ipo::{IpoIssue, IpoReader, IpoSubscription};
pub use market::{
    BlockDeal, CorporateAction, DeliveryBar, InstitutionalFlow, InstrumentSearchResult,
    MacroSeries, MarketBar, MarketDepth, MarketDepthLevel, MarketHistory, MarketIndicators,
    MarketQuote, MarketReader, OiBuildupSnapshot, OptionContract, OptionsChain,
};
pub use portfolio::{AccountSummary, DateRange, OrderFilter, PortfolioReader};
pub use snapshot::{SnapshotData, SnapshotNeeds, SnapshotSource};
pub use strategy_store::{StrategyParameterSpec, StrategySpec, StrategyStore};
pub use tax::{ClosedLotRecord, TaxReportReader};
pub use trade_intent::{TradeIntent, TradeIntentSink, TradeIntentStatus, intent_notional};
pub mod strategy;
pub use strategy::{DefaultStrategyRunner, StrategyRunner};
