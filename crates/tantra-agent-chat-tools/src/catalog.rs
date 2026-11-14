use std::sync::Arc;
use tantra_agent_chat::tool::{ChatTool, ToolRegistry};
use tantra_agent_tools::{RegimeCheckTool, RiskInspectionTool, ScreenerQueryTool};

use crate::alert::{PriceAlertListTool, PriceAlertTool};
use crate::calc::ResearchCalcTool;
use crate::calendar::EconomicCalendarTool;
use crate::content::{FilingsSearchTool, NewsSearchTool};
use crate::fund::{FundHoldingsTool, FundSearchTool};
use crate::fundamental::{FinancialStatementsTool, FundamentalsTool};
use crate::ipo::{IpoCalendarTool, IpoSubscriptionTool};
use crate::lift::{lift_regime, lift_risk, lift_screener};
use crate::market::{
    BlockDealsTool, CorporateActionsTool, DeliveryAnalysisTool, InstitutionalFlowsTool,
    MacroSeriesTool, MarketDepthTool, MarketHistoryTool, MarketIndicatorsTool, MarketQuoteTool,
    MarketSearchTool, OiBuildupTool, OptionsChainTool,
};
use crate::portfolio::{
    OrdersViewTool, PnlViewTool, PortfolioViewTool, PositionsViewTool, TaxLotsViewTool,
};
use crate::ports::alert::{AlertReader, AlertWriter};
use crate::ports::calendar::CalendarReader;
use crate::ports::content::ContentReader;
use crate::ports::fund::FundReader;
use crate::ports::fundamental::FundamentalReader;
use crate::ports::ipo::IpoReader;
use crate::ports::market::MarketReader;
use crate::ports::portfolio::PortfolioReader;
use crate::ports::snapshot::SnapshotSource;
use crate::ports::strategy_store::StrategyStore;
use crate::ports::tax::TaxReportReader;
use crate::ports::trade_intent::TradeIntentSink;
use crate::statement::LedgerStatementTool;
use crate::strategy_manage::StrategyManageTool;
use crate::tax::TaxReportTool;
use crate::trade_intent::ProposeTradeTool;

/// Bundle of all 32 Phase B read-only conversational tools (§6.5).
///
/// The port set grows one `Arc<dyn ...>` reader per tool category; the flat signature
/// deliberately mirrors `register_read_only_tools` and is allowed past clippy's
/// `too_many_arguments` threshold (same precedent as `tantra-agent-chat::turn`).
#[allow(clippy::too_many_arguments)]
pub fn create_read_only_tools(
    portfolio: Arc<dyn PortfolioReader>,
    market: Arc<dyn MarketReader>,
    content: Arc<dyn ContentReader>,
    calendar: Arc<dyn CalendarReader>,
    fundamental: Arc<dyn FundamentalReader>,
    fund: Arc<dyn FundReader>,
    ipo: Arc<dyn IpoReader>,
    tax: Arc<dyn TaxReportReader>,
    snapshots: Option<Arc<dyn SnapshotSource>>,
) -> Vec<Arc<dyn ChatTool>> {
    vec![
        // 1. Market tools (8)
        Arc::new(MarketQuoteTool::new(Arc::clone(&market))),
        Arc::new(MarketHistoryTool::new(Arc::clone(&market))),
        Arc::new(MarketIndicatorsTool::new(Arc::clone(&market))),
        Arc::new(MarketSearchTool::new(Arc::clone(&market))),
        Arc::new(MacroSeriesTool::new(Arc::clone(&market))),
        Arc::new(BlockDealsTool::new(Arc::clone(&market))),
        Arc::new(OptionsChainTool::new(Arc::clone(&market))),
        Arc::new(CorporateActionsTool::new(Arc::clone(&market))),
        // 2. Portfolio tools (5)
        Arc::new(PortfolioViewTool::new(
            Arc::clone(&portfolio),
            Some(Arc::clone(&market)),
        )),
        Arc::new(PositionsViewTool::new(
            Arc::clone(&portfolio),
            Some(Arc::clone(&market)),
        )),
        Arc::new(OrdersViewTool::new(Arc::clone(&portfolio))),
        Arc::new(TaxLotsViewTool::new(Arc::clone(&portfolio))),
        Arc::new(PnlViewTool::new(Arc::clone(&portfolio))),
        // 3. Lifted pure tools from tantra-agent-tools (3)
        Arc::new(lift_screener(
            Arc::new(ScreenerQueryTool),
            snapshots.clone(),
        )),
        Arc::new(lift_regime(Arc::new(RegimeCheckTool), snapshots.clone())),
        Arc::new(lift_risk(Arc::new(RiskInspectionTool), snapshots)),
        // 4. Research calculator (1)
        Arc::new(ResearchCalcTool::new()),
        // 5. News & filings content tools (2, plan 0006 Phase 3)
        Arc::new(NewsSearchTool::new(Arc::clone(&content))),
        Arc::new(FilingsSearchTool::new(content)),
        // 6. Plan 0007 Track 2 Phase 4-5 tools (3)
        Arc::new(MarketDepthTool::new(Arc::clone(&market))),
        Arc::new(InstitutionalFlowsTool::new(Arc::clone(&market))),
        Arc::new(EconomicCalendarTool::new(calendar)),
        // 7. Plan 0010 smart-money tools (2)
        Arc::new(DeliveryAnalysisTool::new(Arc::clone(&market))),
        Arc::new(OiBuildupTool::new(market)),
        // 8. Plan 0007 Track 3 fundamentals / funds / IPO tools (6)
        Arc::new(FundamentalsTool::new(Arc::clone(&fundamental))),
        Arc::new(FinancialStatementsTool::new(Arc::clone(&fundamental))),
        Arc::new(FundSearchTool::new(Arc::clone(&fund))),
        Arc::new(FundHoldingsTool::new(Arc::clone(&fund))),
        Arc::new(IpoCalendarTool::new(Arc::clone(&ipo))),
        Arc::new(IpoSubscriptionTool::new(ipo)),
        // 9. Plan 0007 Track 5 tax / statements (2)
        Arc::new(TaxReportTool::new(Arc::clone(&tax))),
        Arc::new(LedgerStatementTool::new(tax)),
    ]
}

/// Register all 32 Phase B read-only chat tools into a registry (§6.5).
#[allow(clippy::too_many_arguments)]
pub fn register_read_only_tools(
    registry: &mut ToolRegistry,
    portfolio: Arc<dyn PortfolioReader>,
    market: Arc<dyn MarketReader>,
    content: Arc<dyn ContentReader>,
    calendar: Arc<dyn CalendarReader>,
    fundamental: Arc<dyn FundamentalReader>,
    fund: Arc<dyn FundReader>,
    ipo: Arc<dyn IpoReader>,
    tax: Arc<dyn TaxReportReader>,
    snapshots: Option<Arc<dyn SnapshotSource>>,
) {
    let tools = create_read_only_tools(
        portfolio,
        market,
        content,
        calendar,
        fundamental,
        fund,
        ipo,
        tax,
        snapshots,
    );
    registry.register_all(tools);
}

/// Bundle of the 2 price-alert tools (§6.5, plan 0006 Phase 2): the list tool is read-only,
/// the CRUD tool is `ToolRisk::LocalWrite` — platform-managed state, not a read-only query,
/// so it lives outside [`create_read_only_tools`] and its all-`ReadOnly` catalogue-count
/// invariant.
pub fn create_alert_tools(
    reader: Arc<dyn AlertReader>,
    writer: Arc<dyn AlertWriter>,
) -> Vec<Arc<dyn ChatTool>> {
    vec![
        // 1. Price alerts (2: 1 read-only + 1 LocalWrite CRUD)
        Arc::new(PriceAlertListTool::new(reader)),
        Arc::new(PriceAlertTool::new(writer)),
    ]
}

/// Register the 2 price-alert tools into a registry (§6.5).
pub fn register_alert_tools(
    registry: &mut ToolRegistry,
    reader: Arc<dyn AlertReader>,
    writer: Arc<dyn AlertWriter>,
) {
    let tools = create_alert_tools(reader, writer);
    registry.register_all(tools);
}

/// Bundle of the 2 safe trade tools (§6.5, plan 0007 Track 5): `propose_trade`
/// (trade-intent bridge) and `strategy_manage` (strategy CRUD) are both
/// `ToolRisk::LocalWrite` — platform-managed state or gated intents, not read-only
/// queries, so they live outside [`create_read_only_tools`] and its all-`ReadOnly`
/// catalogue-count invariant (same shape as the alert bundle).
pub fn create_trade_tools(
    sink: Arc<dyn TradeIntentSink>,
    store: Arc<dyn StrategyStore>,
) -> Vec<Arc<dyn ChatTool>> {
    vec![
        // 1. Safe trade intent (LocalWrite, approval-queue pattern)
        Arc::new(ProposeTradeTool::new(sink)),
        // 2. Strategy manifest CRUD (LocalWrite)
        Arc::new(StrategyManageTool::new(store)),
    ]
}

/// Register the 2 safe trade tools into a registry (§6.5).
pub fn register_trade_tools(
    registry: &mut ToolRegistry,
    sink: Arc<dyn TradeIntentSink>,
    store: Arc<dyn StrategyStore>,
) {
    let tools = create_trade_tools(sink, store);
    registry.register_all(tools);
}

use crate::ports::strategy::StrategyRunner;
use crate::strategy::{StrategyRunTool, StrategyTuneTool, TradeSuggestionTool};

/// Bundle of the 3 strategy advisory tools (§6.4).
pub fn create_strategy_tools(runner: Option<Arc<dyn StrategyRunner>>) -> Vec<Arc<dyn ChatTool>> {
    if let Some(r) = runner {
        vec![
            Arc::new(StrategyRunTool::with_runner(Arc::clone(&r))),
            Arc::new(StrategyTuneTool::with_runner(Arc::clone(&r))),
            Arc::new(TradeSuggestionTool::with_runner(r)),
        ]
    } else {
        vec![
            Arc::new(StrategyRunTool::new()),
            Arc::new(StrategyTuneTool::new()),
            Arc::new(TradeSuggestionTool::new()),
        ]
    }
}

/// Register the 3 strategy execution and trade suggestion tools into a registry (§6.4).
pub fn register_strategy_tools(
    registry: &mut ToolRegistry,
    runner: Option<Arc<dyn StrategyRunner>>,
) {
    let tools = create_strategy_tools(runner);
    registry.register_all(tools);
}
