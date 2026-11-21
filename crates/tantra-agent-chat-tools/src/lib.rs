//! # tantra-agent-chat-tools
//!
//! Conversational read-only tools and domain read ports for `tantra-agent` (§4, §6).
//!
//! Owns the `PortfolioReader`, `MarketReader`, `ContentReader`, and `SnapshotSource` ports, the
//! lifted pure tools via [`PureToolAdapter`], and model-facing tools for market, portfolio,
//! content, and research calculations.
//!
//! Architectural invariants:
//! - **Pure advisory (NoFX)**: chat tools never place orders or produce `Decision`s (§4.3).
//! - **Dependency rules (R1, R2, R4)**: depends on `tantra-ledger` for types only; zero dependency
//!   on `tantra-runtime`, `tantra-exec`, `tantra-market`, or `tantra-agent-trader-loop`.

pub mod alert;
pub mod calc;
pub mod calendar;
pub mod catalog;
pub mod content;
pub mod experimental_study;
pub mod fund;
pub mod fundamental;
pub mod ipo;
pub mod lift;
pub mod market;
pub mod portfolio;
pub mod ports;
pub mod statement;
pub mod strategy_loop;
pub mod strategy_manage;
pub mod tax;
pub mod trade_intent;

pub use alert::{
    AlertOperation, PriceAlertInput, PriceAlertListInput, PriceAlertListTool, PriceAlertTool,
};
pub use calc::{
    CagrArgs, CalcOperation, MarketSegment, PositionSizingArgs, ResearchCalcInput,
    ResearchCalcTool, ReturnMetricsArgs, StatutoryCostsArgs, TradeSide,
};
pub use calendar::{EconomicCalendarInput, EconomicCalendarTool};
pub use catalog::{
    create_alert_tools, create_read_only_tools, create_read_only_tools_with_studies,
    create_trade_tools, register_alert_tools, register_read_only_tools,
    register_read_only_tools_with_studies, register_trade_tools,
};
pub use content::{FilingsSearchInput, FilingsSearchTool, NewsSearchInput, NewsSearchTool};
pub use experimental_study::{
    ExperimentalStudySummaryInput, ExperimentalStudySummaryTool, HistoricalOutlierNewsInput,
    HistoricalOutlierNewsTool,
};
pub use fund::{FundHoldingsInput, FundHoldingsTool, FundSearchInput, FundSearchTool};
pub use fundamental::{
    FinancialStatementsInput, FinancialStatementsTool, FundamentalsInput, FundamentalsTool,
};
pub use ipo::{IpoCalendarInput, IpoCalendarTool, IpoSubscriptionInput, IpoSubscriptionTool};
pub use lift::{PureToolAdapter, lift_regime, lift_risk, lift_screener};
pub use market::{
    BlockDealsInput, BlockDealsTool, CorporateActionsInput, CorporateActionsTool,
    DeliveryAnalysisInput, DeliveryAnalysisTool, InstitutionalFlowsInput, InstitutionalFlowsTool,
    MacroSeriesInput, MacroSeriesTool, MarketDepthInput, MarketDepthTool, MarketHistoryInput,
    MarketHistoryTool, MarketIndicatorsInput, MarketIndicatorsTool, MarketQuoteInput,
    MarketQuoteTool, MarketSearchInput, MarketSearchTool, OiBuildupInput, OiBuildupTool,
    OptionsChainInput, OptionsChainTool, parse_instrument,
};
pub use portfolio::{
    OrdersViewInput, OrdersViewTool, PnlViewInput, PnlViewTool, PortfolioViewInput,
    PortfolioViewTool, PositionsViewInput, PositionsViewTool, TaxLotsViewInput, TaxLotsViewTool,
};
pub use ports::{
    AccountSummary, AlertReader, AlertStatus, AlertWriter, BlockDeal, CalendarReader,
    ClosedLotRecord, ContentReader, CorporateAction, DateRange, DeliveryBar, EconomicCalendarQuery,
    EconomicEvent, FilingRecord, FinancialStatement, FundHolding, FundReader, FundamentalRatios,
    FundamentalReader, InstitutionalFlow, InstrumentSearchResult, IpoIssue, IpoReader,
    IpoSubscription, MacroSeries, MarketBar, MarketDepth, MarketDepthLevel, MarketHistory,
    MarketIndicators, MarketQuote, MarketReader, MutualFundMeta, NewsArticle, OiBuildupSnapshot,
    OptionContract, OptionsChain, OrderFilter, PortError, PortErrorKind, PortfolioReader,
    PriceAlert, SnapshotData, SnapshotNeeds, SnapshotSource, StrategyParameterSpec, StrategySpec,
    StrategyStore, StudyProbabilityLevel, StudyReader, StudySummarySnapshot, TaxReportReader,
    TradeIntent, TradeIntentSink, TradeIntentStatus, intent_notional,
};
pub mod strategy;

pub use catalog::{
    create_strategy_loop_tools, create_strategy_tools, register_strategy_loop_tools,
    register_strategy_tools,
};
pub use ports::strategy::{DefaultStrategyRunner, StrategyRunner};
pub use statement::{LedgerStatementInput, LedgerStatementTool};
pub use strategy::{
    StrategyRunInput, StrategyRunTool, StrategyTuneInput, StrategyTuneTool, TradeSuggestionInput,
    TradeSuggestionTool,
};
pub use strategy_loop::{
    ChartSceneAnnotateTool, StrategyHedgeRecommendTool, StrategyLoopEvalTool, StrategyLoopSweepTool,
};
pub use strategy_manage::{StrategyManageInput, StrategyManageTool};
pub use tax::{TaxReportInput, TaxReportTool};
pub use trade_intent::{ProposeTradeInput, ProposeTradeTool};
