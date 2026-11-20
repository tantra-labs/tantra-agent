use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tantra_agent_chat::event::{CallId, TurnId};
use tantra_agent_chat::permission::ToolRisk;
use tantra_agent_chat::tool::{ChatTool, ToolContext, ToolRegistry};
use tantra_agent_chat_tools::calc::{
    CagrArgs, CalcOperation, MarketSegment, PositionSizingArgs, ResearchCalcInput,
    ResearchCalcTool, ReturnMetricsArgs, StatutoryCostsArgs, TradeSide,
};
use tantra_agent_chat_tools::catalog::{
    register_read_only_tools, register_read_only_tools_with_studies,
};
use tantra_agent_chat_tools::ports::alert::{AlertReader, AlertStatus, AlertWriter, PriceAlert};
use tantra_agent_chat_tools::ports::calendar::{
    CalendarReader, EconomicCalendarQuery, EconomicEvent,
};
use tantra_agent_chat_tools::ports::content::{ContentReader, FilingRecord, NewsArticle};
use tantra_agent_chat_tools::ports::error::PortError;
use tantra_agent_chat_tools::ports::fund::{FundHolding, FundReader, MutualFundMeta};
use tantra_agent_chat_tools::ports::fundamental::{
    FinancialStatement, FundamentalRatios, FundamentalReader,
};
use tantra_agent_chat_tools::ports::ipo::{IpoIssue, IpoReader, IpoSubscription};
use tantra_agent_chat_tools::ports::market::{
    BlockDeal, CorporateAction, DeliveryBar, InstitutionalFlow, InstrumentSearchResult,
    MacroSeries, MarketBar, MarketDepth, MarketDepthLevel, MarketHistory, MarketIndicators,
    MarketQuote, MarketReader, OiBuildupSnapshot, OiWall, OiWallsSnapshot, OptionContract,
    OptionsChain,
};
use tantra_agent_chat_tools::ports::portfolio::{
    AccountSummary, DateRange, OrderFilter, PortfolioReader,
};
use tantra_agent_chat_tools::ports::snapshot::{SnapshotData, SnapshotNeeds, SnapshotSource};
use tantra_agent_chat_tools::ports::strategy_store::{StrategySpec, StrategyStore};
use tantra_agent_chat_tools::ports::study::{
    StudyProbabilityLevel, StudyReader, StudySummarySnapshot,
};
use tantra_agent_chat_tools::ports::tax::{ClosedLotRecord, TaxReportReader};
use tantra_agent_chat_tools::ports::trade_intent::{
    TradeIntent, TradeIntentSink, TradeIntentStatus,
};
use tantra_agent_chat_tools::statement::format_ledger_statement;
use tantra_agent_chat_tools::tax::compute_capital_gains;
use tantra_agent_chat_tools::trade_intent::trade_intent_to_decision;
use tantra_agent_chat_tools::{create_alert_tools, register_alert_tools};
use tantra_agent_core::ToolError;
use tantra_core::{
    AccountId, AlgoId, ClientOrderId, Exchange, InstrumentId, Level, LevelId, LevelKind, LevelMap,
    LevelSide, Money, OrderSide, OrderStatus, OrderType, PositionSide, Price, PriceBand, Quantity,
    TimeInForce,
};
use tantra_ledger::repository::OrderRecord;
use tantra_ledger::{PositionRecord, TaxLot};
use tokio::sync::mpsc;

// ——————————————————————————————————————————————————————————————————————————
// Mocks for Testing Ports
// ——————————————————————————————————————————————————————————————————————————

struct MockMarketReader {
    fail_quotes: bool,
}

#[async_trait]
impl MarketReader for MockMarketReader {
    async fn quote(&self, instrument: &InstrumentId) -> Result<MarketQuote, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable("market feed offline"));
        }
        Ok(MarketQuote {
            instrument: instrument.clone(),
            last_price: Price(dec!(2500.0)),
            bid: Some(Price(dec!(2499.5))),
            ask: Some(Price(dec!(2500.5))),
            volume: Quantity(dec!(100000.0)),
            as_of: Utc::now(),
        })
    }

    async fn history(
        &self,
        instrument: &InstrumentId,
        _range: DateRange,
        limit: Option<usize>,
    ) -> Result<MarketHistory, PortError> {
        let count = limit.unwrap_or(10);
        let mut bars = Vec::new();
        for i in 0..count {
            bars.push(MarketBar {
                timestamp: Utc::now() - chrono::Duration::minutes((count - i) as i64),
                open: Price(dec!(2490.0)),
                high: Price(dec!(2510.0)),
                low: Price(dec!(2485.0)),
                close: Price(dec!(2500.0)),
                volume: Quantity(dec!(1000.0)),
            });
        }
        Ok(MarketHistory {
            instrument: instrument.clone(),
            bars,
            downsampled: false,
        })
    }

    async fn indicators(
        &self,
        instrument: &InstrumentId,
        names: &[String],
    ) -> Result<MarketIndicators, PortError> {
        let mut values = HashMap::new();
        for name in names {
            if name == "rsi_14" {
                values.insert(name.clone(), dec!(62.45));
            } else if name == "sma_20" {
                values.insert(name.clone(), dec!(2480.0));
            } else {
                values.insert(name.clone(), dec!(100.0));
            }
        }
        Ok(MarketIndicators {
            instrument: instrument.clone(),
            values,
            as_of: Utc::now(),
        })
    }

    async fn search_instruments(
        &self,
        query: &str,
    ) -> Result<Vec<InstrumentSearchResult>, PortError> {
        if query.to_uppercase().contains("RELIANCE") {
            Ok(vec![InstrumentSearchResult {
                instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
                name: "Reliance Industries Limited".to_string(),
                exchange: "NSE".to_string(),
                lot_size: Some(1),
            }])
        } else {
            Ok(vec![])
        }
    }

    async fn macro_series(&self, series: &str) -> Result<MacroSeries, PortError> {
        Ok(MacroSeries {
            series: series.to_string(),
            points: vec![
                (Utc::now() - chrono::Duration::days(30), dec!(5.4)),
                (Utc::now(), dec!(5.1)),
            ],
        })
    }

    async fn block_deals(
        &self,
        instrument: Option<&InstrumentId>,
        date: Option<DateTime<Utc>>,
    ) -> Result<Vec<BlockDeal>, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "block/bulk deal disclosures are not yet ingested",
            ));
        }
        let inst = instrument
            .cloned()
            .unwrap_or_else(|| InstrumentId::new(Exchange::Nse, "RELIANCE"));
        Ok(vec![BlockDeal {
            instrument: inst,
            exchange: "NSE".to_string(),
            client_name: "MORGAN STANLEY ASIA SINGAPORE PTE".to_string(),
            side: "buy".to_string(),
            category: "block".to_string(),
            quantity: Quantity(dec!(500000.0)),
            price: Price(dec!(2500.0)),
            trade_date: date.unwrap_or_else(Utc::now),
        }])
    }

    async fn price_band(&self, _instrument: &InstrumentId) -> Result<PriceBand, PortError> {
        Ok(PriceBand::new(
            Price(dec!(2250.0)),
            Price(dec!(2750.0)),
            Price(dec!(2500.0)),
        ))
    }

    async fn options_chain(
        &self,
        underlying: &InstrumentId,
        expiry: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<OptionsChain, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "options-chain data is not served yet — see tantra-market/docs/gaps/0001-options-chain-reader.md",
            ));
        }
        let mut contracts = vec![
            OptionContract {
                instrument: InstrumentId::new(Exchange::Nfo, "NIFTY26NOV24000CE"),
                option_type: "CE".to_string(),
                strike: Price(dec!(24000.0)),
                expiry: Utc::now() + chrono::Duration::days(14),
                lot_size: Quantity(dec!(75.0)),
                last_price: Some(Price(dec!(210.5))),
                bid: Some(Price(dec!(209.0))),
                ask: Some(Price(dec!(212.0))),
                open_interest: Some(Quantity(dec!(1250000.0))),
            },
            OptionContract {
                instrument: InstrumentId::new(Exchange::Nfo, "NIFTY26NOV24000PE"),
                option_type: "PE".to_string(),
                strike: Price(dec!(24000.0)),
                expiry: Utc::now() + chrono::Duration::days(14),
                lot_size: Quantity(dec!(75.0)),
                last_price: Some(Price(dec!(195.0))),
                bid: None,
                ask: None,
                open_interest: None,
            },
        ];
        if let Some(exp) = expiry {
            contracts.retain(|c| c.expiry.date_naive() == exp.date_naive());
        }
        if let Some(l) = limit {
            contracts.truncate(l);
        }
        Ok(OptionsChain {
            underlying: underlying.clone(),
            as_of: Utc::now(),
            contracts,
        })
    }

    async fn corporate_actions(
        &self,
        instrument: Option<&InstrumentId>,
        action_type: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<CorporateAction>, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "corporate-actions data is Python-side only — see tantra-data/docs/gaps/0008-corporate-actions-rust-bridge.md",
            ));
        }
        let inst = instrument
            .cloned()
            .unwrap_or_else(|| InstrumentId::new(Exchange::Nse, "RELIANCE"));
        let mut actions = vec![
            CorporateAction {
                instrument: inst.clone(),
                action_type: "dividend".to_string(),
                ex_date: Utc::now() - chrono::Duration::days(20),
                record_date: Some(Utc::now() - chrono::Duration::days(21)),
                amount: Some(Money::inr(dec!(4.0))),
                ratio_from: None,
                ratio_to: None,
            },
            CorporateAction {
                instrument: inst,
                action_type: "split".to_string(),
                ex_date: Utc::now() - chrono::Duration::days(90),
                record_date: None,
                amount: None,
                ratio_from: Some(Quantity(dec!(1.0))),
                ratio_to: Some(Quantity(dec!(2.0))),
            },
        ];
        if let Some(ty) = action_type {
            actions.retain(|a| a.action_type == ty);
        }
        if let Some(l) = limit {
            actions.truncate(l);
        }
        Ok(actions)
    }

    async fn market_depth(
        &self,
        instrument: &InstrumentId,
        _levels: Option<usize>,
    ) -> Result<MarketDepth, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "L2 order-book depth source not wired — see tantra-market/docs/gaps/0006-l2-order-book-depth.md",
            ));
        }
        let bid_total = Quantity(dec!(1200.0));
        let ask_total = Quantity(dec!(800.0));
        Ok(MarketDepth {
            instrument: instrument.clone(),
            as_of: Utc::now(),
            bids: vec![
                MarketDepthLevel {
                    price: Price(dec!(2499.0)),
                    quantity: Quantity(dec!(700.0)),
                },
                MarketDepthLevel {
                    price: Price(dec!(2498.0)),
                    quantity: Quantity(dec!(500.0)),
                },
            ],
            asks: vec![
                MarketDepthLevel {
                    price: Price(dec!(2501.0)),
                    quantity: Quantity(dec!(500.0)),
                },
                MarketDepthLevel {
                    price: Price(dec!(2502.0)),
                    quantity: Quantity(dec!(300.0)),
                },
            ],
            // (1200 - 800) / (1200 + 800) * 100 = 20.
            liquidity_imbalance_pct: (bid_total.0 - ask_total.0) / (bid_total.0 + ask_total.0)
                * dec!(100.0),
        })
    }

    async fn institutional_flows(
        &self,
        _date: Option<DateTime<Utc>>,
    ) -> Result<Vec<InstitutionalFlow>, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "institutional-flow reader not wired; see tantra-runtime/docs/gaps/0007 item 11",
            ));
        }
        Ok(vec![InstitutionalFlow {
            category: "fii".to_string(),
            buy_value: Money::inr(dec!(12000.0)),
            sell_value: Money::inr(dec!(8000.0)),
            net_value: Money::inr(dec!(4000.0)),
            trade_date: Utc::now(),
        }])
    }

    async fn delivery_stats(
        &self,
        instrument: &InstrumentId,
        _range: DateRange,
        limit: Option<usize>,
    ) -> Result<Vec<DeliveryBar>, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "delivery reader not wired; see tantra-runtime/docs/gaps/0007 item 12",
            ));
        }
        let mut bars = vec![
            DeliveryBar {
                instrument: instrument.clone(),
                date: Utc::now() - chrono::Duration::days(2),
                traded_qty: Quantity(dec!(1000000.0)),
                delivered_qty: Quantity(dec!(720000.0)),
                delivery_pct: dec!(72.0),
            },
            DeliveryBar {
                instrument: instrument.clone(),
                date: Utc::now() - chrono::Duration::days(1),
                traded_qty: Quantity(dec!(1200000.0)),
                delivered_qty: Quantity(dec!(900000.0)),
                delivery_pct: dec!(75.0),
            },
        ];
        if let Some(l) = limit {
            bars.truncate(l);
        }
        Ok(bars)
    }

    async fn oi_buildup(
        &self,
        instrument: &InstrumentId,
        _lookback_days: Option<u32>,
    ) -> Result<OiBuildupSnapshot, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "no open-interest store wired in the runtime chat adapters — see tantra-runtime/docs/gaps/0007 item 12",
            ));
        }
        Ok(OiBuildupSnapshot {
            instrument: instrument.clone(),
            as_of: Utc::now(),
            open_interest: Quantity(dec!(1250000.0)),
            oi_change_pct: Some(dec!(6.5)),
            price_change_pct: Some(dec!(2.1)),
            classification: "long_buildup".to_string(),
        })
    }

    async fn level_map(
        &self,
        _instrument: &InstrumentId,
        _date: Option<NaiveDate>,
    ) -> Result<LevelMap, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "level-map reader not wired; see tantra-market/docs/gaps/0005-elephant-zones-oi-levels-and-orderblock-indicators.md",
            ));
        }
        Ok(LevelMap::new(vec![
            Level::new(
                LevelId(1),
                LevelKind::ElephantResistance1,
                LevelSide::Resistance,
                (dec!(2515.0), dec!(2525.0)),
                3,
            )
            .expect("valid level band and score"),
            Level::new(
                LevelId(2),
                LevelKind::MaxPain,
                LevelSide::Resistance,
                (dec!(25000.0), dec!(25000.0)),
                2,
            )
            .expect("valid level band and score"),
        ]))
    }

    async fn oi_walls(
        &self,
        _instrument: &InstrumentId,
        _expiry: Option<NaiveDate>,
    ) -> Result<OiWallsSnapshot, PortError> {
        if self.fail_quotes {
            return Err(PortError::unavailable(
                "OI-wall reader not wired; see tantra-data/docs/gaps/0016-intraday-option-chain-snapshot-lakehouse.md",
            ));
        }
        Ok(OiWallsSnapshot {
            top_call_walls: vec![OiWall {
                strike: Price(dec!(25000.0)),
                oi: dec!(1250000.0),
                delta_oi_score: dec!(0.82),
            }],
            top_put_walls: vec![OiWall {
                strike: Price(dec!(24800.0)),
                oi: dec!(980000.0),
                delta_oi_score: dec!(0.64),
            }],
            local_pcr: dec!(0.78),
            as_of: Utc::now(),
        })
    }
}

struct MockContentReader {
    fail_content: bool,
}

#[async_trait]
impl ContentReader for MockContentReader {
    async fn news_search(
        &self,
        query: Option<&str>,
        instrument: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<NewsArticle>, PortError> {
        if self.fail_content {
            return Err(PortError::unavailable("news content feed offline"));
        }
        let articles = vec![NewsArticle {
            id: "news_001".to_string(),
            title: "Reliance quarterly results beat estimates".to_string(),
            source: "Economic Times".to_string(),
            url: Some("https://example.com/news_001".to_string()),
            published_at: Utc::now(),
            summary: Some("Q2 results above street expectations".to_string()),
            instruments: vec!["NSE:RELIANCE".to_string()],
        }];
        let take = limit.unwrap_or(articles.len());
        let articles: Vec<NewsArticle> = articles
            .into_iter()
            .filter(|a| query.is_none_or(|q| a.title.to_lowercase().contains(&q.to_lowercase())))
            .filter(|a| {
                instrument
                    .is_none_or(|i| a.instruments.iter().any(|tag| tag.eq_ignore_ascii_case(i)))
            })
            .take(take)
            .collect();
        Ok(articles)
    }

    async fn filings(
        &self,
        instrument: Option<&InstrumentId>,
        filing_type: Option<&str>,
        _limit: Option<usize>,
    ) -> Result<Vec<FilingRecord>, PortError> {
        if self.fail_content {
            return Err(PortError::unavailable("filings source offline"));
        }
        let inst = instrument
            .cloned()
            .unwrap_or_else(|| InstrumentId::new(Exchange::Nse, "RELIANCE"));
        let filings = vec![FilingRecord {
            instrument: inst,
            filing_date: Utc::now(),
            filing_type: "quarterly_result".to_string(),
            title: "Quarterly results filing".to_string(),
            source_url: Some("https://example.com/filing".to_string()),
        }];
        let filings: Vec<FilingRecord> = filings
            .into_iter()
            .filter(|f| filing_type.is_none_or(|t| f.filing_type == t))
            .collect();
        Ok(filings)
    }
}

struct MockCalendarReader;

#[async_trait]
impl CalendarReader for MockCalendarReader {
    async fn economic_calendar(
        &self,
        query: &EconomicCalendarQuery,
    ) -> Result<Vec<EconomicEvent>, PortError> {
        let events = vec![
            EconomicEvent {
                id: "ec_001".to_string(),
                country: "IN".to_string(),
                title: "RBI Repo Rate Decision".to_string(),
                currency: Some("INR".to_string()),
                impact: "high".to_string(),
                forecast: Some("6.50%".to_string()),
                previous: Some("6.50%".to_string()),
                actual: Some("6.25%".to_string()),
                event_date: Utc::now() - chrono::Duration::days(2),
            },
            EconomicEvent {
                id: "ec_002".to_string(),
                country: "US".to_string(),
                title: "US CPI YoY".to_string(),
                currency: Some("USD".to_string()),
                impact: "medium".to_string(),
                forecast: Some("3.1%".to_string()),
                previous: Some("3.2%".to_string()),
                actual: None,
                event_date: Utc::now() + chrono::Duration::days(3),
            },
        ];
        let mut hits: Vec<EconomicEvent> = events
            .into_iter()
            .filter(|e| query.country.as_deref().is_none_or(|c| e.country == c))
            .filter(|e| query.impact.as_deref().is_none_or(|i| e.impact == i))
            .filter(|e| query.start.is_none_or(|s| e.event_date >= s))
            .filter(|e| query.end.is_none_or(|end| e.event_date < end))
            .collect();
        if let Some(limit) = query.limit {
            hits.truncate(limit);
        }
        Ok(hits)
    }
}

struct MockFundamentalReader {
    fail: bool,
}

#[async_trait]
impl FundamentalReader for MockFundamentalReader {
    async fn fundamental_ratios(
        &self,
        instrument: &InstrumentId,
    ) -> Result<FundamentalRatios, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "fundamentals data not ingested yet — see tantra-data/docs/gaps/0020-fundamentals-ingestion.md",
            ));
        }
        Ok(FundamentalRatios {
            instrument: instrument.clone(),
            as_of: Utc::now(),
            pe_ratio: Some(dec!(18.5)),
            pb_ratio: Some(dec!(3.2)),
            ev_ebitda: Some(dec!(12.4)),
            dividend_yield_pct: Some(dec!(0.8)),
            roce_pct: Some(dec!(15.2)),
            roe_pct: Some(dec!(12.0)),
            debt_equity: Some(dec!(0.4)),
            interest_coverage: Some(dec!(9.1)),
        })
    }

    async fn financial_statements(
        &self,
        instrument: &InstrumentId,
        statement_type: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<FinancialStatement>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "fundamentals data not ingested yet — see tantra-data/docs/gaps/0020-fundamentals-ingestion.md",
            ));
        }
        let statements = vec![
            FinancialStatement {
                instrument: instrument.clone(),
                statement_type: "income_statement".to_string(),
                fiscal_quarter: "FY2026-Q2".to_string(),
                revenue: Some(Money::inr(dec!(2500000000.0))),
                operating_profit: Some(Money::inr(dec!(450000000.0))),
                net_income: Some(Money::inr(dec!(300000000.0))),
                eps: Some(dec!(18.9)),
            },
            FinancialStatement {
                instrument: instrument.clone(),
                statement_type: "balance_sheet".to_string(),
                fiscal_quarter: "FY2026-Q2".to_string(),
                revenue: None,
                operating_profit: None,
                net_income: None,
                eps: None,
            },
        ];
        let mut hits: Vec<FinancialStatement> = statements
            .into_iter()
            .filter(|s| statement_type.is_none_or(|t| s.statement_type == t))
            .collect();
        if let Some(l) = limit {
            hits.truncate(l);
        }
        Ok(hits)
    }
}

struct MockFundReader {
    fail: bool,
}

#[async_trait]
impl FundReader for MockFundReader {
    async fn fund_search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<MutualFundMeta>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "mutual-fund data not ingested yet — see tantra-data/docs/gaps/0021-mutual-fund-data-ingestion.md",
            ));
        }
        let funds = vec![MutualFundMeta {
            scheme_code: "122639".to_string(),
            scheme_name: "Parag Parikh Flexi Cap Fund".to_string(),
            category: "Flexi Cap".to_string(),
            nav: Some(Money::inr(dec!(78.5))),
            cagr_1y_pct: Some(dec!(24.1)),
            cagr_3y_pct: Some(dec!(18.2)),
            cagr_5y_pct: Some(dec!(16.8)),
            expense_ratio_pct: Some(dec!(0.61)),
        }];
        let hits: Vec<MutualFundMeta> = funds
            .into_iter()
            .filter(|f| {
                f.scheme_name.to_lowercase().contains(&query.to_lowercase())
                    || f.scheme_code.contains(query)
            })
            .collect();
        let take = limit.unwrap_or(hits.len());
        Ok(hits.into_iter().take(take).collect())
    }

    async fn fund_holdings(&self, scheme_code: &str) -> Result<Vec<FundHolding>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "mutual-fund data not ingested yet — see tantra-data/docs/gaps/0021-mutual-fund-data-ingestion.md",
            ));
        }
        if scheme_code != "122639" {
            return Ok(vec![]);
        }
        Ok(vec![
            FundHolding {
                asset_name: "RELIANCE".to_string(),
                weight_pct: dec!(8.4),
                sector: Some("Energy".to_string()),
            },
            FundHolding {
                asset_name: "TCS".to_string(),
                weight_pct: dec!(6.9),
                sector: Some("IT".to_string()),
            },
        ])
    }
}

struct MockIpoReader {
    fail: bool,
}

#[async_trait]
impl IpoReader for MockIpoReader {
    async fn upcoming_ipos(&self, limit: Option<usize>) -> Result<Vec<IpoIssue>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "IPO data not ingested yet — see tantra-data/docs/gaps/0022-ipo-calendar-and-subscription-ingestion.md",
            ));
        }
        let issues = vec![IpoIssue {
            symbol: "ZAGGLECOM".to_string(),
            company_name: "Zaggle Prepaid Ocean Services".to_string(),
            open_date: Utc::now() + chrono::Duration::days(2),
            close_date: Utc::now() + chrono::Duration::days(4),
            price_min: Some(Price(dec!(140.0))),
            price_max: Some(Price(dec!(148.0))),
            lot_size: Some(100),
            issue_size: Some(Money::inr(dec!(8000000000.0))),
        }];
        let take = limit.unwrap_or(issues.len());
        Ok(issues.into_iter().take(take).collect())
    }

    async fn ipo_subscription(&self, symbol: &str) -> Result<IpoSubscription, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "IPO data not ingested yet — see tantra-data/docs/gaps/0022-ipo-calendar-and-subscription-ingestion.md",
            ));
        }
        Ok(IpoSubscription {
            symbol: symbol.to_string(),
            as_of: Utc::now(),
            qib_multiple: Some(dec!(2.5)),
            nii_multiple: Some(dec!(1.8)),
            rii_multiple: Some(dec!(3.2)),
            total_multiple: Some(dec!(2.6)),
        })
    }
}

struct MockTradeIntentSink {
    fail: bool,
}

#[async_trait]
impl TradeIntentSink for MockTradeIntentSink {
    async fn propose_trade(&self, intent: &TradeIntent) -> Result<TradeIntentStatus, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "trade-intent sink not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 17",
            ));
        }
        let status = if intent.paper_trading {
            "paper_filled"
        } else {
            "pending_approval"
        };
        Ok(TradeIntentStatus {
            intent_id: "INT-1001".to_string(),
            status: status.to_string(),
            message: format!("intent for `{}` enqueued", intent.instrument),
        })
    }
}

struct MockStrategyStore {
    fail: bool,
    strategies: std::sync::Mutex<Vec<StrategySpec>>,
}

impl MockStrategyStore {
    fn new(fail: bool) -> Self {
        Self {
            fail,
            strategies: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl StrategyStore for MockStrategyStore {
    async fn save_strategy(&self, spec: &StrategySpec) -> Result<StrategySpec, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "strategy store not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 18",
            ));
        }
        let mut all = self.strategies.lock().unwrap();
        if let Some(existing) = all.iter_mut().find(|s| s.strategy_id == spec.strategy_id) {
            *existing = spec.clone();
        } else {
            all.push(spec.clone());
        }
        Ok(spec.clone())
    }

    async fn list_strategies(&self) -> Result<Vec<StrategySpec>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "strategy store not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 18",
            ));
        }
        Ok(self.strategies.lock().unwrap().clone())
    }

    async fn get_strategy(&self, strategy_id: &str) -> Result<StrategySpec, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "strategy store not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 18",
            ));
        }
        self.strategies
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.strategy_id == strategy_id)
            .cloned()
            .ok_or_else(|| PortError::not_found(format!("strategy `{strategy_id}` not found")))
    }

    async fn delete_strategy(&self, strategy_id: &str) -> Result<bool, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "strategy store not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 18",
            ));
        }
        let mut all = self.strategies.lock().unwrap();
        let before = all.len();
        all.retain(|s| s.strategy_id != strategy_id);
        Ok(all.len() != before)
    }
}

struct MockTaxReportReader {
    fail: bool,
}

#[async_trait]
impl TaxReportReader for MockTaxReportReader {
    async fn closed_lots(
        &self,
        _account: &AccountId,
        _range: DateRange,
    ) -> Result<Vec<ClosedLotRecord>, PortError> {
        if self.fail {
            return Err(PortError::unavailable(
                "matched-lot bridge not wired in the runtime — see tantra-runtime/docs/gaps/0007 item 19",
            ));
        }
        Ok(vec![
            ClosedLotRecord {
                lot_id: "LOT-ST-1".to_string(),
                instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
                quantity: Quantity(dec!(100.0)),
                entry_price: Price(dec!(2000.0)),
                exit_price: Price(dec!(2500.0)),
                realized_pnl: Money::inr(dec!(50000.0)),
                holding_period_days: 90,
                is_long_term: false,
            },
            ClosedLotRecord {
                lot_id: "LOT-LT-1".to_string(),
                instrument: InstrumentId::new(Exchange::Nse, "TCS"),
                quantity: Quantity(dec!(10.0)),
                entry_price: Price(dec!(3000.0)),
                exit_price: Price(dec!(3400.0)),
                realized_pnl: Money::inr(dec!(4000.0)),
                holding_period_days: 500,
                is_long_term: true,
            },
        ])
    }
}

struct MockPortfolioReader;

#[async_trait]
impl PortfolioReader for MockPortfolioReader {
    async fn accounts(&self) -> Result<Vec<AccountSummary>, PortError> {
        Ok(vec![AccountSummary {
            account_id: AccountId::new("ACC-01"),
            algo_id: Some(AlgoId::new("ALPHA-1")),
            cash: Money::inr(dec!(500000.0)),
            status: "active".to_string(),
            updated_at: Utc::now(),
        }])
    }

    async fn positions(&self, account: &AccountId) -> Result<Vec<PositionRecord>, PortError> {
        if account.as_str() != "ACC-01" {
            return Ok(vec![]);
        }
        Ok(vec![PositionRecord {
            account_id: account.clone(),
            instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
            side: PositionSide::Long,
            quantity: Quantity(dec!(100.0)),
            avg_price: Price(dec!(2400.0)),
            realized_pnl: Money::inr(dec!(5000.0)),
            unrealized_pnl: Money::inr(dec!(10000.0)),
            lots: vec![TaxLot {
                lot_id: "LOT-01".to_string(),
                account_id: account.clone(),
                instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
                side: PositionSide::Long,
                entry_price: Price(dec!(2400.0)),
                quantity: Quantity(dec!(100.0)),
                remaining_quantity: Quantity(dec!(100.0)),
                entry_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                created_at: Utc::now(),
            }],
            updated_at: Utc::now(),
        }])
    }

    async fn tax_lots(
        &self,
        account: &AccountId,
        _instrument: &InstrumentId,
    ) -> Result<Vec<TaxLot>, PortError> {
        Ok(vec![TaxLot {
            lot_id: "LOT-01".to_string(),
            account_id: account.clone(),
            instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
            side: PositionSide::Long,
            entry_price: Price(dec!(2400.0)),
            quantity: Quantity(dec!(100.0)),
            remaining_quantity: Quantity(dec!(100.0)),
            entry_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            created_at: Utc::now(),
        }])
    }

    async fn cash(&self, _account: &AccountId) -> Result<Vec<Money>, PortError> {
        Ok(vec![Money::inr(dec!(500000.0))])
    }

    async fn orders(
        &self,
        account: &AccountId,
        _filter: OrderFilter,
    ) -> Result<Vec<OrderRecord>, PortError> {
        Ok(vec![OrderRecord {
            client_order_id: ClientOrderId::new("ORD-101"),
            broker_order_id: Some("BROKER-101".to_string()),
            algo_id: Some(AlgoId::new("ALPHA-1")),
            account_id: account.clone(),
            instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit(Price(dec!(2400.0))),
            time_in_force: TimeInForce::Day,
            price: Some(Price(dec!(2400.0))),
            quantity: Quantity(dec!(100.0)),
            filled_quantity: Quantity(dec!(100.0)),
            avg_fill_price: Some(Price(dec!(2400.0))),
            status: OrderStatus::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }])
    }

    async fn realized_pnl(
        &self,
        _account: &AccountId,
        _range: DateRange,
    ) -> Result<Money, PortError> {
        Ok(Money::inr(dec!(25000.0)))
    }
}

struct MockSnapshotSource;

#[async_trait]
impl SnapshotSource for MockSnapshotSource {
    async fn snapshot(&self, _needs: &SnapshotNeeds) -> Result<SnapshotData, PortError> {
        Ok(SnapshotData {
            as_of: Utc::now(),
            universe: Some(vec!["RELIANCE".to_string(), "TCS".to_string()]),
            regime: Some("trending".to_string()),
            risk_metrics: HashMap::new(),
        })
    }
}

/// In-memory alert store implementing both alert port traits over one shared `Vec` — the same
/// contract the runtime's profile-backed store implements (duplicate ids rejected; a
/// `Triggered` alert is never flipped by enable/disable).
struct MockAlertStore {
    alerts: std::sync::Mutex<Vec<PriceAlert>>,
}

impl MockAlertStore {
    fn new() -> Self {
        Self {
            alerts: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl AlertReader for MockAlertStore {
    async fn list_alerts(&self, workspace: Option<&str>) -> Result<Vec<PriceAlert>, PortError> {
        let all = self.alerts.lock().unwrap().clone();
        Ok(match workspace {
            Some(w) => all.into_iter().filter(|a| a.workspace_id == w).collect(),
            None => all,
        })
    }
}

#[async_trait]
impl AlertWriter for MockAlertStore {
    async fn add_alert(&self, alert: PriceAlert) -> Result<PriceAlert, PortError> {
        let mut alerts = self.alerts.lock().unwrap();
        if alerts.iter().any(|a| a.id == alert.id) {
            return Err(PortError::invalid_argument(format!(
                "an alert with id `{}` already exists",
                alert.id
            )));
        }
        alerts.push(alert.clone());
        Ok(alert)
    }

    async fn set_alert_status(
        &self,
        id: &str,
        status: AlertStatus,
    ) -> Result<Option<PriceAlert>, PortError> {
        let mut alerts = self.alerts.lock().unwrap();
        Ok(match alerts.iter_mut().find(|a| a.id == id) {
            Some(alert) => {
                if alert.status != AlertStatus::Triggered {
                    alert.status = status;
                }
                Some(alert.clone())
            }
            None => None,
        })
    }

    async fn delete_alert(&self, id: &str) -> Result<Option<PriceAlert>, PortError> {
        let mut alerts = self.alerts.lock().unwrap();
        let removed = alerts
            .iter()
            .position(|a| a.id == id)
            .map(|i| alerts.remove(i));
        Ok(removed)
    }
}

fn dummy_tool_context() -> ToolContext {
    let (tx, _) = mpsc::channel(16);
    ToolContext::new(None, TurnId(1), CallId::new("call-1"), tx)
}

fn parse_dec(val: &serde_json::Value) -> f64 {
    if let Some(s) = val.as_str() {
        s.parse::<f64>().unwrap_or(0.0)
    } else {
        val.as_f64().unwrap_or(0.0)
    }
}

// ——————————————————————————————————————————————————————————————————————————
// Unit Tests
// ——————————————————————————————————————————————————————————————————————————

#[tokio::test]
async fn test_research_calc_cagr() {
    let tool = ResearchCalcTool::new();
    let ctx = dummy_tool_context();

    let input = serde_json::to_value(ResearchCalcInput {
        operation: CalcOperation::Cagr,
        cagr: Some(CagrArgs {
            start_value: dec!(100000.0),
            end_value: dec!(200000.0),
            years: dec!(3.0),
        }),
        position_sizing: None,
        statutory_costs: None,
        return_metrics: None,
    })
    .unwrap();

    let out = tool.execute(input, ctx).await.unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["operation"], "cagr");
    let cagr = parse_dec(&out.payload["cagr_percent"]);
    // (2)^(1/3) - 1 ≈ 25.9921%
    assert!((cagr - 25.9921).abs() < 0.01);
}

#[tokio::test]
async fn test_research_calc_position_sizing() {
    let tool = ResearchCalcTool::new();
    let ctx = dummy_tool_context();

    let input = serde_json::to_value(ResearchCalcInput {
        operation: CalcOperation::PositionSizing,
        cagr: None,
        position_sizing: Some(PositionSizingArgs {
            capital: dec!(500000.0),
            risk_percent: dec!(1.0), // 1%
            entry_price: dec!(2500.0),
            stop_loss: dec!(2450.0), // Rs 50 risk per share
            lot_size: Some(10),
        }),
        statutory_costs: None,
        return_metrics: None,
    })
    .unwrap();

    let out = tool.execute(input, ctx).await.unwrap();
    assert!(out.ok);
    assert_eq!(parse_dec(&out.payload["risk_capital"]), 5000.0);
    assert_eq!(parse_dec(&out.payload["per_share_risk"]), 50.0);
    // 5000 / 50 = 100 shares (multiple of 10 lot size)
    assert_eq!(out.payload["quantity"], 100);
}

#[tokio::test]
async fn test_research_calc_statutory_costs() {
    let tool = ResearchCalcTool::new();
    let ctx = dummy_tool_context();

    let input = serde_json::to_value(ResearchCalcInput {
        operation: CalcOperation::StatutoryCosts,
        cagr: None,
        position_sizing: None,
        statutory_costs: Some(StatutoryCostsArgs {
            segment: MarketSegment::Delivery,
            side: TradeSide::Buy,
            notional: dec!(100000.0),
            brokerage: Some(Decimal::ZERO),
        }),
        return_metrics: None,
    })
    .unwrap();

    let out = tool.execute(input, ctx).await.unwrap();
    assert!(out.ok);
    assert_eq!(parse_dec(&out.payload["charges"]["stt"]), 100.0); // 0.1%
    assert_eq!(parse_dec(&out.payload["charges"]["stamp_duty"]), 15.0); // 0.015%
    let total = parse_dec(&out.payload["total_cost"]);
    assert!(total > 118.0 && total < 125.0);
}

#[tokio::test]
async fn test_research_calc_return_metrics() {
    let tool = ResearchCalcTool::new();
    let ctx = dummy_tool_context();

    let input = serde_json::to_value(ResearchCalcInput {
        operation: CalcOperation::ReturnMetrics,
        cagr: None,
        position_sizing: None,
        statutory_costs: None,
        return_metrics: Some(ReturnMetricsArgs {
            cost_basis: dec!(100000.0),
            current_value: dec!(120000.0),
            cash_flow: Some(dec!(5000.0)),
            days_held: Some(365),
        }),
    })
    .unwrap();

    let out = tool.execute(input, ctx).await.unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["operation"], "return_metrics");
    assert_eq!(parse_dec(&out.payload["capital_gain"]), 20000.0);
    assert_eq!(parse_dec(&out.payload["total_profit"]), 25000.0);
    assert_eq!(parse_dec(&out.payload["return_percent"]), 25.0);
}

#[tokio::test]
async fn test_market_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    let portfolio = Arc::new(MockPortfolioReader);
    let snapshot = Arc::new(MockSnapshotSource);

    register_read_only_tools(
        &mut registry,
        portfolio,
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        Some(snapshot),
    );

    // 1. market_quote
    let quote_tool = registry
        .get("market_quote")
        .expect("market_quote registered");
    let out = quote_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(parse_dec(&out.payload["last_price"]), 2500.0);

    // 2. market_history
    let hist_tool = registry
        .get("market_history")
        .expect("market_history registered");
    let out = hist_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "limit": 5 }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 5);

    // 3. market_indicators
    let ind_tool = registry
        .get("market_indicators")
        .expect("market_indicators registered");
    let out = ind_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "indicators": ["rsi_14", "sma_20"] }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["indicators"]["rsi_14"], "62.45");

    // 4. market_search
    let search_tool = registry
        .get("market_search")
        .expect("market_search registered");
    let out = search_tool
        .execute(
            serde_json::json!({ "query": "RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);

    // 5. macro_series
    let macro_tool = registry
        .get("macro_series")
        .expect("macro_series registered");
    let out = macro_tool
        .execute(serde_json::json!({ "series": "cpi" }), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["series"], "cpi");
}

#[tokio::test]
async fn test_block_deals_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. block_deals — success, no filters (defaults to latest trading day).
    let tool = registry.get("block_deals").expect("block_deals registered");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);
    let out = tool
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["deals"][0]["instrument"], "NSE:RELIANCE");
    assert_eq!(out.payload["deals"][0]["side"], "buy");
    assert_eq!(out.payload["deals"][0]["category"], "block");
    assert_eq!(parse_dec(&out.payload["deals"][0]["price"]), 2500.0);
    assert_eq!(parse_dec(&out.payload["deals"][0]["quantity"]), 500000.0);

    // 2. block_deals — instrument + date filters parsed through to the port.
    let out = tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:TCS",
                "date": "2026-11-10T10:00:00Z"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["deals"][0]["instrument"], "NSE:TCS");
    assert!(out.payload["deals"][0]["trade_date"].is_string());

    // 3. block_deals — honest unavailability when no disclosure source exists.
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("block_deals")
        .unwrap()
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable port must surface as ToolError::Execution, got {err:?}"
    );
}

#[tokio::test]
async fn test_content_tools() {
    let content = Arc::new(MockContentReader {
        fail_content: false,
    });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        content,
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. news_search — both tools registered and read-only.
    let news = registry.get("news_search").expect("news_search registered");
    assert_eq!(news.traits().risk, ToolRisk::ReadOnly);
    let filings = registry
        .get("filings_search")
        .expect("filings_search registered");
    assert_eq!(filings.traits().risk, ToolRisk::ReadOnly);

    // 2. news_search — success, no filters.
    let out = news
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["articles"][0]["id"], "news_001");
    assert_eq!(
        out.payload["articles"][0]["title"],
        "Reliance quarterly results beat estimates"
    );
    assert_eq!(out.payload["articles"][0]["instruments"][0], "NSE:RELIANCE");
    assert_eq!(out.payload["articles"][0]["source"], "Economic Times");
    assert!(out.payload["articles"][0]["published_at"].is_string());

    // 3. news_search — query + instrument-mention filters parsed through to the port.
    let out = news
        .execute(
            serde_json::json!({
                "query": "results",
                "instrument": "NSE:RELIANCE",
                "limit": 5
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);

    // 4. news_search — non-matching query yields an empty result, not an error.
    let out = news
        .execute(
            serde_json::json!({ "query": "unrelated" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 0);

    // 5. filings_search — success with type filter.
    let out = filings
        .execute(
            serde_json::json!({
                "instrument": "NSE:TCS",
                "filing_type": "quarterly_result"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["filings"][0]["instrument"], "NSE:TCS");
    assert_eq!(out.payload["filings"][0]["filing_type"], "quarterly_result");
    assert!(out.payload["filings"][0]["filing_date"].is_string());

    // 6. filings_search — non-matching type filter yields an empty result, not an error.
    let out = filings
        .execute(
            serde_json::json!({ "filing_type": "insider_trade" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 0);

    // 7. filings_search — unknown exchange rejected as invalid arguments.
    let err = filings
        .execute(
            serde_json::json!({ "instrument": "BOGUS:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown exchange must surface as ToolError::InvalidArguments, got {err:?}"
    );

    // 8. Content tools — honest unavailability when no content source exists.
    let failing = Arc::new(MockContentReader { fail_content: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        failing,
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    for name in ["news_search", "filings_search"] {
        let err = fail_registry
            .get(name)
            .unwrap()
            .execute(serde_json::json!({}), dummy_tool_context())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution(..)),
            "unavailable content port must surface as ToolError::Execution, got {err:?}"
        );
    }

    // 9. news_search — deny_unknown_fields rejects unexpected arguments.
    let err = registry
        .get("news_search")
        .unwrap()
        .execute(serde_json::json!({ "bogus": true }), dummy_tool_context())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn test_options_and_corporate_actions_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. Both tools registered and read-only.
    let chain_tool = registry
        .get("options_chain")
        .expect("options_chain registered");
    assert_eq!(chain_tool.traits().risk, ToolRisk::ReadOnly);
    let actions_tool = registry
        .get("corporate_actions")
        .expect("corporate_actions registered");
    assert_eq!(actions_tool.traits().risk, ToolRisk::ReadOnly);

    // 2. options_chain — success, no filters (defaults to full chain).
    let out = chain_tool
        .execute(
            serde_json::json!({ "underlying": "NSE:NIFTY" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["underlying"], "NSE:NIFTY");
    assert_eq!(out.payload["count"], 2);
    let ce = &out.payload["contracts"][0];
    assert_eq!(ce["instrument"], "NFO:NIFTY26NOV24000CE");
    assert_eq!(ce["option_type"], "CE");
    assert_eq!(parse_dec(&ce["strike"]), 24000.0);
    assert_eq!(parse_dec(&ce["lot_size"]), 75.0);
    assert_eq!(parse_dec(&ce["last_price"]), 210.5);
    assert_eq!(parse_dec(&ce["open_interest"]), 1250000.0);
    assert!(ce["expiry"].is_string());
    let pe = &out.payload["contracts"][1];
    assert_eq!(pe["option_type"], "PE");
    assert!(pe["bid"].is_null());
    assert!(pe["open_interest"].is_null());

    // 3. options_chain — limit caps the returned contracts.
    let out = chain_tool
        .execute(
            serde_json::json!({ "underlying": "NSE:NIFTY", "limit": 1 }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);

    // 4. options_chain — unknown exchange rejected as invalid arguments.
    let err = chain_tool
        .execute(
            serde_json::json!({ "underlying": "BOGUS:NIFTY" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown exchange must surface as ToolError::InvalidArguments, got {err:?}"
    );

    // 5. options_chain — deny_unknown_fields rejects unexpected arguments.
    let err = chain_tool
        .execute(
            serde_json::json!({ "underlying": "NSE:NIFTY", "bogus": true }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );

    // 6. corporate_actions — success, no filters.
    let out = actions_tool
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    assert_eq!(out.payload["actions"][0]["instrument"], "NSE:RELIANCE");
    assert_eq!(out.payload["actions"][0]["action_type"], "dividend");
    assert_eq!(parse_dec(&out.payload["actions"][0]["amount"]), 4.0);
    assert!(out.payload["actions"][0]["ex_date"].is_string());
    assert!(out.payload["actions"][0]["record_date"].is_string());
    assert_eq!(out.payload["actions"][1]["action_type"], "split");
    assert!(out.payload["actions"][1]["amount"].is_null());
    assert_eq!(parse_dec(&out.payload["actions"][1]["ratio_to"]), 2.0);

    // 7. corporate_actions — action-type filter parsed through to the port.
    let out = actions_tool
        .execute(
            serde_json::json!({ "action_type": "split" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);
    let out = actions_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "action_type": "bonus" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 0);

    // 8. Honest unavailability when no upstream source/bridge exists.
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    for (name, input) in [
        (
            "options_chain",
            serde_json::json!({ "underlying": "NSE:NIFTY" }),
        ),
        ("corporate_actions", serde_json::json!({})),
    ] {
        let err = fail_registry
            .get(name)
            .unwrap()
            .execute(input, dummy_tool_context())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution(..)),
            "unavailable port must surface as ToolError::Execution, got {err:?}"
        );
    }
}

#[tokio::test]
async fn test_alert_tools() {
    let store = Arc::new(MockAlertStore::new());
    let reader: Arc<dyn AlertReader> = store.clone();
    let writer: Arc<dyn AlertWriter> = store.clone();
    let mut registry = ToolRegistry::new();
    register_alert_tools(&mut registry, reader, writer);

    // 1. Risk tiers: the list tool is read-only, the CRUD tool is LocalWrite.
    let list = registry
        .get("price_alert_list")
        .expect("price_alert_list registered");
    assert_eq!(list.traits().risk, ToolRisk::ReadOnly);
    let crud = registry.get("price_alert").expect("price_alert registered");
    assert_eq!(crud.traits().risk, ToolRisk::LocalWrite);

    // 2. list on an empty store → count 0.
    let out = list
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 0);

    // 3. add → active alert with a minted id, decimal-string price, default workspace.
    let out = crud
        .execute(
            serde_json::json!({
                "operation": "add",
                "instrument": "NSE:RELIANCE",
                "price": "2500.50"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    let alert = &out.payload["alert"];
    let added_id = alert["id"].as_str().unwrap().to_string();
    assert!(added_id.starts_with("al_"));
    assert_eq!(alert["instrumentId"], "NSE:RELIANCE");
    assert_eq!(alert["price"], "2500.50");
    assert_eq!(alert["status"], "active");
    assert_eq!(alert["workspaceId"], "default");
    assert_eq!(alert["triggeredAt"], serde_json::Value::Null);

    // 4. list sees it; instrument + status filters and workspace scoping work.
    let out = list
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);
    let out = list
        .execute(
            serde_json::json!({ "instrument": "RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);
    let out = list
        .execute(
            serde_json::json!({ "instrument": "NSE:TCS" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 0);
    let out = list
        .execute(
            serde_json::json!({ "status": "active" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);
    let out = list
        .execute(
            serde_json::json!({ "workspace": "other" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 0);

    // 5. disable → paused; enable → active.
    let out = crud
        .execute(
            serde_json::json!({ "operation": "disable", "id": added_id }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["alert"]["status"], "paused");
    let out = crud
        .execute(
            serde_json::json!({ "operation": "enable", "id": added_id }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["alert"]["status"], "active");

    // 6. enable/disable on a triggered alert is a no-op (port contract, ui-web parity).
    store.alerts.lock().unwrap()[0].status = AlertStatus::Triggered;
    let out = crud
        .execute(
            serde_json::json!({ "operation": "disable", "id": added_id }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["alert"]["status"], "triggered");

    // 7. delete → removed; deleting again is an explicit not-found error.
    let out = crud
        .execute(
            serde_json::json!({ "operation": "delete", "id": added_id }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["alert"]["id"], added_id.as_str());
    let err = crud
        .execute(
            serde_json::json!({ "operation": "delete", "id": added_id }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unknown id must surface as ToolError::Execution, got {err:?}"
    );

    // 8. Input validation.
    let err = crud
        .execute(
            serde_json::json!({ "operation": "add", "price": "10" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
    let err = crud
        .execute(
            serde_json::json!({ "operation": "add", "instrument": "NSE:RELIANCE", "price": "-1" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
    let err = crud
        .execute(
            serde_json::json!({ "operation": "delete" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
    let err = list
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "extra": true }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));

    // 9. The alert bundle is exactly 2 tools (list + CRUD).
    let reader: Arc<dyn AlertReader> = Arc::new(MockAlertStore::new());
    let writer: Arc<dyn AlertWriter> = Arc::new(MockAlertStore::new());
    assert_eq!(create_alert_tools(reader, writer).len(), 2);
}

#[tokio::test]
async fn test_portfolio_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let portfolio = Arc::new(MockPortfolioReader);
    let content = Arc::new(MockContentReader {
        fail_content: false,
    });
    let mut registry = ToolRegistry::new();

    register_read_only_tools(
        &mut registry,
        portfolio,
        market,
        content,
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. portfolio_view
    let pv_tool = registry
        .get("portfolio_view")
        .expect("portfolio_view registered");
    let out = pv_tool
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["total_accounts"], 1);
    // Cost basis: 100 * 2400 = 240,000; Market value: 100 * 2500 = 250,000; Unrealized = 10,000
    assert_eq!(
        parse_dec(&out.payload["accounts"][0]["cost_basis"]),
        240000.0
    );
    assert_eq!(
        parse_dec(&out.payload["accounts"][0]["valuation"]["market_value"]),
        250000.0
    );
    assert_eq!(
        parse_dec(&out.payload["accounts"][0]["valuation"]["unrealized_pnl"]),
        10000.0
    );

    // 2. positions_view
    let pos_tool = registry
        .get("positions_view")
        .expect("positions_view registered");
    let out = pos_tool
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(parse_dec(&out.payload["positions"][0]["quantity"]), 100.0);

    // 3. orders_view
    let orders_tool = registry.get("orders_view").expect("orders_view registered");
    let out = orders_tool
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["orders"][0]["client_order_id"], "ORD-101");

    // 4. taxlots_view
    let taxlots_tool = registry
        .get("taxlots_view")
        .expect("taxlots_view registered");
    let out = taxlots_tool
        .execute(
            serde_json::json!({ "account": "ACC-01", "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["lots"][0]["lot_id"], "LOT-01");

    // 5. pnl_view
    let pnl_tool = registry.get("pnl_view").expect("pnl_view registered");
    let out = pnl_tool
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(parse_dec(&out.payload["realized_pnl"]), 25000.0);
}

#[tokio::test]
async fn test_portfolio_graceful_degradation_without_quotes() {
    let market = Arc::new(MockMarketReader { fail_quotes: true });
    let portfolio = Arc::new(MockPortfolioReader);
    let mut registry = ToolRegistry::new();

    let content = Arc::new(MockContentReader {
        fail_content: false,
    });
    register_read_only_tools(
        &mut registry,
        portfolio,
        market,
        content,
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let pv_tool = registry.get("portfolio_view").unwrap();
    let out = pv_tool
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();

    assert!(out.ok);
    assert_eq!(out.payload["accounts"][0]["valuation"], "unavailable");
    assert_eq!(
        parse_dec(&out.payload["accounts"][0]["cost_basis"]),
        240000.0
    );
}

#[tokio::test]
async fn test_lifted_pure_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let portfolio = Arc::new(MockPortfolioReader);
    let snapshot = Arc::new(MockSnapshotSource);
    let mut registry = ToolRegistry::new();

    register_read_only_tools(
        &mut registry,
        portfolio,
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        Some(snapshot),
    );

    // 1. screener_scan
    let screener = registry.get("screener_scan").expect("screener_scan lifted");
    assert_eq!(screener.traits().risk, ToolRisk::ReadOnly);
    let out = screener
        .execute(
            serde_json::json!({ "universe": ["INFY", "TCS"], "metric": "rsi_14" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["universe_size"], 2);

    // 2. regime_check
    let regime = registry.get("regime_check").expect("regime_check lifted");
    assert_eq!(regime.traits().risk, ToolRisk::ReadOnly);
    let out = regime
        .execute(
            serde_json::json!({ "regime": "trending" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["regime"], "trending");

    // 3. risk_inspect
    let risk = registry.get("risk_inspect").expect("risk_inspect lifted");
    assert_eq!(risk.traits().risk, ToolRisk::ReadOnly);
    let out = risk
        .execute(
            serde_json::json!({ "metric": "price_band" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["metric"], "price_band");
}

#[tokio::test]
async fn test_total_read_only_catalogue_count() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let portfolio = Arc::new(MockPortfolioReader);
    let content = Arc::new(MockContentReader {
        fail_content: false,
    });
    let mut registry = ToolRegistry::new();

    register_read_only_tools(
        &mut registry,
        portfolio,
        market,
        content,
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let specs = registry.specs();
    assert_eq!(specs.len(), 36);

    for spec in &specs {
        let tool = registry.get(&spec.name).unwrap();
        assert_eq!(
            tool.traits().risk,
            ToolRisk::ReadOnly,
            "Tool `{}` must have ReadOnly risk level",
            spec.name
        );
    }
}

#[tokio::test]
async fn test_strategy_chat_tools_execution() {
    use tantra_agent_chat_tools::catalog::{create_strategy_tools, register_strategy_tools};
    use tantra_agent_chat_tools::strategy::{
        StrategyRunTool, StrategyTuneTool, TradeSuggestionTool,
    };

    // 1. StrategyRunTool
    let run_tool = StrategyRunTool::new();
    assert_eq!(run_tool.spec().name, "strategy_run");
    assert_eq!(run_tool.traits().risk, ToolRisk::ReadOnly);

    let run_out = run_tool
        .execute(
            serde_json::json!({
                "strategy": "turtle_breakout",
                "symbol": "NSE:NIFTY50-INDEX",
                "capital": "1000000"
            }),
            dummy_tool_context(),
        )
        .await
        .expect("strategy_run should succeed");

    assert!(run_out.ok);
    assert_eq!(run_out.payload["strategy"], "turtle_breakout");
    assert_eq!(run_out.payload["total_trades"], 12);
    assert!(run_out.payload["net_pnl"].is_number() || run_out.payload["net_pnl"].is_string());

    // 2. StrategyTuneTool
    let tune_tool = StrategyTuneTool::new();
    assert_eq!(tune_tool.spec().name, "strategy_tune");
    assert_eq!(tune_tool.traits().risk, ToolRisk::ReadOnly);

    let tune_out = tune_tool
        .execute(
            serde_json::json!({
                "strategy": "turtle_breakout",
                "symbol": "NSE:NIFTY50-INDEX",
                "metric": "sharpe"
            }),
            dummy_tool_context(),
        )
        .await
        .expect("strategy_tune should succeed");

    assert!(tune_out.ok);
    assert_eq!(tune_out.payload["strategy"], "turtle_breakout");
    assert!(tune_out.payload["tuned_params"].is_object());
    assert!(
        tune_out.payload["overfitting_verdict"]
            .as_str()
            .unwrap()
            .contains("Overfitting Risk")
    );

    // 3. TradeSuggestionTool
    let sugg_tool = TradeSuggestionTool::new();
    assert_eq!(sugg_tool.spec().name, "strategy_suggest");
    assert_eq!(sugg_tool.traits().risk, ToolRisk::ReadOnly);

    let sugg_out = sugg_tool
        .execute(
            serde_json::json!({
                "strategy": "turtle_breakout",
                "symbol": "NSE:NIFTY50-INDEX",
                "risk_pct": "1.0"
            }),
            dummy_tool_context(),
        )
        .await
        .expect("strategy_suggest should succeed");

    assert!(sugg_out.ok);
    assert_eq!(sugg_out.payload["signal"]["bias"], "BUY / LONG");
    assert!(sugg_out.payload["sizing"]["quantity"].as_u64().unwrap() > 0);
    assert!(
        sugg_out.payload["statutory_charges"]["total_charges"].is_number()
            || sugg_out.payload["statutory_charges"]["total_charges"].is_string()
    );

    // 4. Registration in ToolRegistry
    let mut reg = ToolRegistry::new();
    register_strategy_tools(&mut reg, None);
    assert!(reg.get("strategy_run").is_some());
    assert!(reg.get("strategy_tune").is_some());
    assert!(reg.get("strategy_suggest").is_some());

    let bundle = create_strategy_tools(None);
    assert_eq!(bundle.len(), 3);
}

#[tokio::test]
async fn market_depth_tool_returns_ladder() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let tool = registry
        .get("market_depth")
        .expect("market_depth registered");
    assert_eq!(tool.spec().name, "market_depth");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);

    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "levels": 2 }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["instrument"], "NSE:RELIANCE");
    let bids = out.payload["bids"].as_array().unwrap();
    let asks = out.payload["asks"].as_array().unwrap();
    assert_eq!(bids.len(), 2, "best 2 bid levels returned");
    assert_eq!(asks.len(), 2, "best 2 ask levels returned");
    assert_eq!(parse_dec(&bids[0]["price"]), 2499.0);
    assert_eq!(parse_dec(&bids[0]["quantity"]), 700.0);
    assert_eq!(parse_dec(&asks[0]["price"]), 2501.0);
    // bid_total 1200, ask_total 800; imbalance (1200-800)/(1200+800)*100 = 20.
    assert_eq!(parse_dec(&out.payload["liquidity_imbalance_pct"]), 20.0);
    assert_eq!(parse_dec(&out.payload["bid_total"]), 1200.0);
    assert_eq!(parse_dec(&out.payload["ask_total"]), 800.0);
    assert!(out.payload["as_of"].is_string());

    // Honest unavailability when no depth source exists (same pattern as the other
    // market tools with `fail_quotes`).
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("market_depth")
        .unwrap()
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable depth port must surface as ToolError::Execution, got {err:?}"
    );
}

#[tokio::test]
async fn institutional_flows_tool_returns_flows() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let tool = registry
        .get("institutional_flows")
        .expect("institutional_flows registered");
    assert_eq!(tool.spec().name, "institutional_flows");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);

    let out = tool
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    let flow = &out.payload["flows"][0];
    assert_eq!(flow["category"], "fii");
    assert_eq!(parse_dec(&flow["buy_value"]), 12000.0);
    assert_eq!(parse_dec(&flow["sell_value"]), 8000.0);
    assert_eq!(parse_dec(&flow["net_value"]), 4000.0);
    assert!(flow["trade_date"].is_string());
}

#[tokio::test]
async fn economic_calendar_tool_filters_and_limits() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let tool = registry
        .get("economic_calendar")
        .expect("economic_calendar registered");
    assert_eq!(tool.spec().name, "economic_calendar");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);

    // No filters → both fixture events.
    let out = tool
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    assert_eq!(out.payload["events"][0]["id"], "ec_001");
    assert_eq!(out.payload["events"][0]["title"], "RBI Repo Rate Decision");
    assert_eq!(out.payload["events"][0]["impact"], "high");

    // Country + impact filters parsed through to the port.
    let out = tool
        .execute(
            serde_json::json!({ "country": "US", "impact": "medium" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    assert_eq!(out.payload["events"][0]["country"], "US");
    assert_eq!(out.payload["events"][0]["currency"], "USD");

    // Limit caps the returned events.
    let out = tool
        .execute(serde_json::json!({ "limit": 1 }), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);

    // deny_unknown_fields rejects unexpected arguments.
    let err = tool
        .execute(serde_json::json!({ "bogus": true }), dummy_tool_context())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn delivery_analysis_tool_returns_delivery_bars() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let tool = registry
        .get("delivery_analysis")
        .expect("delivery_analysis registered");
    assert_eq!(tool.spec().name, "delivery_analysis");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);

    // 1. Success, no filters — both seeded days returned.
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    let day = &out.payload["delivery"][1];
    assert_eq!(day["instrument"], "NSE:RELIANCE");
    assert_eq!(parse_dec(&day["traded_qty"]), 1200000.0);
    assert_eq!(parse_dec(&day["delivered_qty"]), 900000.0);
    assert_eq!(parse_dec(&day["delivery_pct"]), 75.0);
    assert!(day["date"].is_string());

    // 2. limit caps the returned days.
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "limit": 1 }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);

    // 3. Honest unavailability when no Rust bridge exists.
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("delivery_analysis")
        .unwrap()
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable delivery port must surface as ToolError::Execution, got {err:?}"
    );

    // 4. deny_unknown_fields rejects unexpected arguments.
    let err = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "bogus": true }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn oi_buildup_tool_returns_classification() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let tool = registry.get("oi_buildup").expect("oi_buildup registered");
    assert_eq!(tool.spec().name, "oi_buildup");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);

    // 1. Success — default lookback parsed through to the port.
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NFO:NIFTY26NOV24000CE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["instrument"], "NFO:NIFTY26NOV24000CE");
    assert_eq!(parse_dec(&out.payload["open_interest"]), 1250000.0);
    assert_eq!(parse_dec(&out.payload["oi_change_pct"]), 6.5);
    assert_eq!(parse_dec(&out.payload["price_change_pct"]), 2.1);
    assert_eq!(out.payload["classification"], "long_buildup");
    assert!(out.payload["as_of"].is_string());

    // 2. Honest unavailability when no OI store is wired.
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("oi_buildup")
        .unwrap()
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable OI port must surface as ToolError::Execution, got {err:?}"
    );

    // 3. deny_unknown_fields rejects unexpected arguments.
    let err = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "bogus": true }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn level_map_and_oi_walls_tools() {
    let market = Arc::new(MockMarketReader { fail_quotes: false });
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        market,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    let map_tool = registry.get("level_map").expect("level_map registered");
    assert_eq!(map_tool.spec().name, "level_map");
    assert_eq!(map_tool.traits().risk, ToolRisk::ReadOnly);
    let walls_tool = registry.get("oi_walls").expect("oi_walls registered");
    assert_eq!(walls_tool.spec().name, "oi_walls");
    assert_eq!(walls_tool.traits().risk, ToolRisk::ReadOnly);

    // 1. level_map — success, no date filter; levels carry kind/side/band/score.
    let out = map_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["instrument"], "NSE:NIFTY");
    assert_eq!(out.payload["levels"][0]["id"], 1);
    assert_eq!(out.payload["levels"][0]["kind"], "elephant_resistance1");
    assert_eq!(out.payload["levels"][0]["side"], "resistance");
    assert_eq!(parse_dec(&out.payload["levels"][0]["band"][0]), 2515.0);
    assert_eq!(parse_dec(&out.payload["levels"][0]["band"][1]), 2525.0);
    assert_eq!(out.payload["levels"][0]["score"], 3);
    assert_eq!(out.payload["levels"][1]["kind"], "max_pain");

    // 2. level_map — date parsed through to the port.
    let out = map_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY", "date": "2026-11-17" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["levels"][0]["id"], 1);

    // 3. oi_walls — success; walls and local PCR serialized.
    let out = walls_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["instrument"], "NSE:NIFTY");
    assert_eq!(
        parse_dec(&out.payload["top_call_walls"][0]["strike"]),
        25000.0
    );
    assert_eq!(
        parse_dec(&out.payload["top_call_walls"][0]["oi"]),
        1250000.0
    );
    assert_eq!(
        parse_dec(&out.payload["top_call_walls"][0]["delta_oi_score"]),
        0.82
    );
    assert_eq!(
        parse_dec(&out.payload["top_put_walls"][0]["strike"]),
        24800.0
    );
    assert_eq!(
        parse_dec(&out.payload["top_put_walls"][0]["delta_oi_score"]),
        0.64
    );
    assert_eq!(parse_dec(&out.payload["local_pcr"]), 0.78);
    assert!(out.payload["as_of"].is_string());

    // 4. oi_walls — expiry parsed through to the port.
    let out = walls_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY", "expiry": "2026-11-26" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(
        parse_dec(&out.payload["top_call_walls"][0]["strike"]),
        25000.0
    );

    // 5. Honest unavailability when no level-map / OI-wall source is wired.
    let failing = Arc::new(MockMarketReader { fail_quotes: true });
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        failing,
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    for name in ["level_map", "oi_walls"] {
        let err = fail_registry
            .get(name)
            .unwrap()
            .execute(
                serde_json::json!({ "instrument": "NSE:NIFTY" }),
                dummy_tool_context(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution(..)),
            "unavailable {name} port must surface as ToolError::Execution, got {err:?}"
        );
    }

    // 6. deny_unknown_fields rejects unexpected arguments.
    let err = map_tool
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY", "bogus": true }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidArguments { .. }),
        "unknown field must be rejected, got {err:?}"
    );
}

#[tokio::test]
async fn fundamentals_and_statements_tools() {
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. fundamental_ratios — success.
    let tool = registry
        .get("fundamental_ratios")
        .expect("fundamental_ratios registered");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["instrument"], "NSE:RELIANCE");
    assert_eq!(parse_dec(&out.payload["pe_ratio"]), 18.5);
    assert_eq!(parse_dec(&out.payload["roe_pct"]), 12.0);
    assert!(out.payload["as_of"].is_string());

    // 2. financial_statements — success + type filter + limit cap.
    let tool = registry
        .get("financial_statements")
        .expect("financial_statements registered");
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    assert_eq!(
        out.payload["statements"][0]["statement_type"],
        "income_statement"
    );
    assert_eq!(out.payload["statements"][0]["fiscal_quarter"], "FY2026-Q2");
    assert_eq!(parse_dec(&out.payload["statements"][0]["eps"]), 18.9);
    let out = tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "statement_type": "balance_sheet"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);
    let out = tool
        .execute(
            serde_json::json!({ "instrument": "NSE:RELIANCE", "limit": 1 }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(out.payload["count"], 1);

    // 3. Honest unavailability when no fundamentals source exists.
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: true }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    for name in ["fundamental_ratios", "financial_statements"] {
        let err = fail_registry
            .get(name)
            .unwrap()
            .execute(
                serde_json::json!({ "instrument": "NSE:RELIANCE" }),
                dummy_tool_context(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution(..)),
            "unavailable fundamentals port must surface as ToolError::Execution, got {err:?}"
        );
    }
}

#[tokio::test]
async fn fund_search_and_holdings_tools() {
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. fund_search — success.
    let tool = registry.get("fund_search").expect("fund_search registered");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);
    let out = tool
        .execute(
            serde_json::json!({ "query": "parag" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    let fund = &out.payload["funds"][0];
    assert_eq!(fund["scheme_code"], "122639");
    assert_eq!(fund["category"], "Flexi Cap");
    assert_eq!(parse_dec(&fund["nav"]), 78.5);
    assert_eq!(parse_dec(&fund["cagr_3y_pct"]), 18.2);

    // 2. fund_holdings — success.
    let tool = registry
        .get("fund_holdings")
        .expect("fund_holdings registered");
    let out = tool
        .execute(
            serde_json::json!({ "scheme_code": "122639" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    assert_eq!(out.payload["holdings"][0]["asset_name"], "RELIANCE");
    assert_eq!(parse_dec(&out.payload["holdings"][0]["weight_pct"]), 8.4);

    // 3. Honest unavailability when no MF source exists.
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: true }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("fund_search")
        .unwrap()
        .execute(
            serde_json::json!({ "query": "parag" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable fund port must surface as ToolError::Execution, got {err:?}"
    );
}

#[tokio::test]
async fn ipo_calendar_and_subscription_tools() {
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );

    // 1. ipo_calendar — success.
    let tool = registry
        .get("ipo_calendar")
        .expect("ipo_calendar registered");
    assert_eq!(tool.traits().risk, ToolRisk::ReadOnly);
    let out = tool
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 1);
    let issue = &out.payload["issues"][0];
    assert_eq!(issue["symbol"], "ZAGGLECOM");
    assert_eq!(parse_dec(&issue["price_min"]), 140.0);
    assert_eq!(parse_dec(&issue["price_max"]), 148.0);
    assert_eq!(issue["lot_size"], 100);
    assert!(issue["open_date"].is_string());
    assert!(issue["close_date"].is_string());

    // 2. ipo_subscription — success.
    let tool = registry
        .get("ipo_subscription")
        .expect("ipo_subscription registered");
    let out = tool
        .execute(
            serde_json::json!({ "symbol": "ZAGGLECOM" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["symbol"], "ZAGGLECOM");
    assert_eq!(parse_dec(&out.payload["qib_multiple"]), 2.5);
    assert_eq!(parse_dec(&out.payload["total_multiple"]), 2.6);

    // 3. Honest unavailability when no IPO source exists.
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: true }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let err = fail_registry
        .get("ipo_calendar")
        .unwrap()
        .execute(serde_json::json!({}), dummy_tool_context())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable IPO port must surface as ToolError::Execution, got {err:?}"
    );
}

#[tokio::test]
async fn test_propose_trade_tool() {
    use tantra_agent_chat_tools::catalog::{create_trade_tools, register_trade_tools};
    use tantra_core::decision::Decision;

    // 1. Pure intent → Decision formulation (closed grammar).
    let intent = TradeIntent {
        algo_id: "momentum-alpha-v1".to_string(),
        instrument: InstrumentId::new(Exchange::Nse, "RELIANCE"),
        side: "buy".to_string(),
        order_type: "limit".to_string(),
        quantity: Quantity(dec!(100.0)),
        price: Some(Price(dec!(2500.0))),
        paper_trading: false,
        rationale: Some("test".to_string()),
    };
    let decision = trade_intent_to_decision(&intent).unwrap();
    assert!(matches!(
        decision,
        Decision::PlaceOrder {
            symbol,
            side: OrderSide::Buy,
            qty,
            price_limit: Some(_),
            algo_id,
            ..
        } if symbol == "RELIANCE" && qty.0 == dec!(100.0) && algo_id == "momentum-alpha-v1"
    ));

    // 2. Validation rejects bad intents.
    let bad_qty = TradeIntent {
        quantity: Quantity(dec!(0.0)),
        ..intent.clone()
    };
    assert!(trade_intent_to_decision(&bad_qty).is_err());
    let bad_side = TradeIntent {
        side: "hold".to_string(),
        ..intent.clone()
    };
    assert!(trade_intent_to_decision(&bad_side).is_err());
    let limit_without_price = TradeIntent {
        price: None,
        ..intent.clone()
    };
    assert!(trade_intent_to_decision(&limit_without_price).is_err());

    // 3. Tool submission — approval vs paper mode.
    let sink = Arc::new(MockTradeIntentSink { fail: false });
    let store = Arc::new(MockStrategyStore::new(false));
    let mut registry = ToolRegistry::new();
    register_trade_tools(&mut registry, sink.clone(), store);
    let tool = registry
        .get("propose_trade")
        .expect("propose_trade registered");
    assert_eq!(tool.traits().risk, ToolRisk::LocalWrite);

    let out = tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "side": "buy",
                "order_type": "limit",
                "quantity": "100",
                "price": "2500",
                "algo_id": "momentum-alpha-v1"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["status"], "pending_approval");
    assert_eq!(out.payload["intent_id"], "INT-1001");

    let out = tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "side": "sell",
                "order_type": "market",
                "quantity": "50",
                "algo_id": "momentum-alpha-v1",
                "paper_trading": true
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["status"], "paper_filled");

    // 4. Invalid arguments rejected (bad algo id, non-positive quantity, unknown field).
    for bad in [
        serde_json::json!({
            "instrument": "NSE:RELIANCE",
            "side": "buy",
            "order_type": "market",
            "quantity": "10",
            "algo_id": "x"
        }),
        serde_json::json!({
            "instrument": "NSE:RELIANCE",
            "side": "buy",
            "order_type": "market",
            "quantity": "0",
            "algo_id": "momentum-alpha-v1"
        }),
        serde_json::json!({ "bogus": true }),
    ] {
        let err = tool.execute(bad, dummy_tool_context()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments { .. }),
            "bad input must surface as ToolError::InvalidArguments, got {err:?}"
        );
    }

    // 5. Honest unavailability when no sink is wired.
    let mut fail_registry = ToolRegistry::new();
    register_trade_tools(
        &mut fail_registry,
        Arc::new(MockTradeIntentSink { fail: true }),
        Arc::new(MockStrategyStore::new(false)),
    );
    let err = fail_registry
        .get("propose_trade")
        .unwrap()
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "side": "buy",
                "order_type": "market",
                "quantity": "10",
                "algo_id": "momentum-alpha-v1"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable sink must surface as ToolError::Execution, got {err:?}"
    );

    // 6. The trade bundle is exactly 2 LocalWrite tools.
    let bundle = create_trade_tools(sink, Arc::new(MockStrategyStore::new(false)));
    assert_eq!(bundle.len(), 2);
}

#[tokio::test]
async fn test_strategy_manage_tool() {
    use tantra_agent_chat_tools::catalog::register_trade_tools;

    let store = Arc::new(MockStrategyStore::new(false));
    let mut registry = ToolRegistry::new();
    register_trade_tools(
        &mut registry,
        Arc::new(MockTradeIntentSink { fail: false }),
        store.clone(),
    );
    let tool = registry
        .get("strategy_manage")
        .expect("strategy_manage registered");
    assert_eq!(tool.traits().risk, ToolRisk::LocalWrite);

    // 1. create → inspect → list → delete round-trip.
    let create = tool
        .execute(
            serde_json::json!({
                "operation": "create",
                "strategy_id": "turtle_breakout",
                "name": "Turtle Breakout",
                "instruments": "NSE:NIFTY50-INDEX",
                "parameters": [{
                    "name": "entry",
                    "default": "20",
                    "min": "5",
                    "max": "50",
                    "step": "1"
                }]
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(create.ok);
    assert_eq!(create.payload["strategy"]["strategy_id"], "turtle_breakout");

    let inspect = tool
        .execute(
            serde_json::json!({ "operation": "inspect", "strategy_id": "turtle_breakout" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(inspect.ok);
    assert_eq!(
        inspect.payload["strategy"]["instruments"][0],
        "NSE:NIFTY50-INDEX"
    );
    assert_eq!(
        inspect.payload["strategy"]["parameters"][0]["name"],
        "entry"
    );

    let list = tool
        .execute(
            serde_json::json!({ "operation": "list" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(list.ok);
    assert_eq!(list.payload["count"], 1);

    let delete = tool
        .execute(
            serde_json::json!({ "operation": "delete", "strategy_id": "turtle_breakout" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(delete.ok);
    assert_eq!(delete.payload["deleted"], true);

    // 2. Validation: create without instruments / unknown operation / unknown field.
    let err = tool
        .execute(
            serde_json::json!({ "operation": "create", "strategy_id": "s1" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));
    let err = tool
        .execute(
            serde_json::json!({ "operation": "explode" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }));

    // 3. Honest unavailability when no store is wired.
    let mut fail_registry = ToolRegistry::new();
    register_trade_tools(
        &mut fail_registry,
        Arc::new(MockTradeIntentSink { fail: false }),
        Arc::new(MockStrategyStore::new(true)),
    );
    let err = fail_registry
        .get("strategy_manage")
        .unwrap()
        .execute(
            serde_json::json!({ "operation": "list" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Execution(..)),
        "unavailable store must surface as ToolError::Execution, got {err:?}"
    );
}

#[tokio::test]
async fn test_tax_and_statement_tools() {
    // 1. Pure tax math on the mock closed lots: STCG 50_000 → 20% = 10_000;
    //    LTCG 4_000 is under the 1.25L exemption → no LTCG tax.
    let lots = MockTaxReportReader { fail: false }
        .closed_lots(&AccountId::new("ACC-01"), DateRange::new(None, None))
        .await
        .unwrap();
    let summary = compute_capital_gains(&lots);
    assert_eq!(summary.stcg_pnl.amount, dec!(50000.0));
    assert_eq!(summary.ltcg_pnl.amount, dec!(4000.0));
    assert_eq!(summary.stcg_tax.amount, dec!(10000.0));
    assert_eq!(summary.ltcg_taxable.amount, Decimal::ZERO);
    assert_eq!(summary.ltcg_tax.amount, Decimal::ZERO);
    assert_eq!(summary.total_tax.amount, dec!(10000.0));

    // 2. Statement formatting is a markdown table containing the lots + summary.
    let statement = format_ledger_statement(&lots);
    assert!(statement.contains("| LOT-ST-1 |"));
    assert!(statement.contains("| LOT-LT-1 |"));
    assert!(statement.contains("STCG tax (20%): 10000"));

    // 3. Tool surface via the read-only bundle.
    let mut registry = ToolRegistry::new();
    register_read_only_tools(
        &mut registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let tax = registry.get("tax_report").expect("tax_report registered");
    assert_eq!(tax.traits().risk, ToolRisk::ReadOnly);
    let out = tax
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["count"], 2);
    assert_eq!(parse_dec(&out.payload["summary"]["total_tax"]), 10000.0);

    let stmt = registry
        .get("ledger_statement")
        .expect("ledger_statement registered");
    let out = stmt
        .execute(
            serde_json::json!({ "account": "ACC-01" }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert!(
        out.payload["statement"]
            .as_str()
            .unwrap()
            .contains("LOT-ST-1")
    );

    // 4. Honest unavailability when no matched-lot bridge exists.
    let mut fail_registry = ToolRegistry::new();
    register_read_only_tools(
        &mut fail_registry,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: true }),
        None,
    );
    for name in ["tax_report", "ledger_statement"] {
        let err = fail_registry
            .get(name)
            .unwrap()
            .execute(
                serde_json::json!({ "account": "ACC-01" }),
                dummy_tool_context(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution(..)),
            "unavailable tax port must surface as ToolError::Execution, got {err:?}"
        );
    }
}

struct MockStudyReader {
    fail: bool,
}

#[async_trait]
impl StudyReader for MockStudyReader {
    async fn returns_study_summary(
        &self,
        instrument: &InstrumentId,
        asset_class: Option<&str>,
        period: Option<&str>,
        calc_type: Option<&str>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<StudySummarySnapshot, PortError> {
        if self.fail {
            return Err(PortError::unavailable("study engine unavailable"));
        }
        Ok(StudySummarySnapshot {
            instrument: instrument.clone(),
            asset_class: asset_class.unwrap_or("index").to_string(),
            period_schedule: period.unwrap_or("weekly_expiry").to_string(),
            calc_type: calc_type.unwrap_or("open_to_close").to_string(),
            start_date: start_date.unwrap_or_else(|| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: end_date.unwrap_or_else(|| NaiveDate::from_ymd_opt(2025, 10, 1).unwrap()),
            total_periods: 90,
            regime_note: Some("NSE Sep 2025 Tue cutover applied".to_string()),
            avg_return_pct: dec!(0.34),
            std_dev_pct: dec!(1.42),
            sharpe_ratio: dec!(0.24),
            sortino_ratio: dec!(0.36),
            profit_factor: dec!(1.65),
            max_gain_pct: dec!(4.15),
            max_loss_pct: dec!(-3.80),
            win_rate_pct: dec!(61.11),
            current_streak_direction: Some("positive".to_string()),
            current_streak_length: 2,
            current_streak_return_pct: dec!(1.45),
            current_markov_state: Some("ShortPositive".to_string()),
            strongest_next_markov_state: "ShortNegative".to_string(),
            strongest_next_probability_pct: dec!(58.33),
            probability_levels: vec![
                StudyProbabilityLevel {
                    prob_above_pct: 80,
                    return_pct: dec!(-1.10),
                    implied_price: dec!(24725.00),
                    scenario: "Support / Covered PE".to_string(),
                },
                StudyProbabilityLevel {
                    prob_above_pct: 20,
                    return_pct: dec!(1.55),
                    implied_price: dec!(25387.50),
                    scenario: "Covered Call OTM".to_string(),
                },
            ],
            outlier_90_lower_pct: dec!(-2.25),
            outlier_90_upper_pct: dec!(2.40),
            outlier_count: 9,
            after_big_up_next_avg_pct: Some(dec!(0.18)),
            after_big_down_next_avg_pct: Some(dec!(0.62)),
        })
    }
}

#[tokio::test]
async fn test_experimental_study_and_outlier_news_tools() {
    // 1. Default read-only bundle registers experimental_study_summary + historical_outlier_news,
    //    and experimental_study_summary returns honest unavailability when StudyReader is None.
    let mut default_reg = ToolRegistry::new();
    register_read_only_tools(
        &mut default_reg,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
    );
    let summary_unwired = default_reg
        .get("experimental_study_summary")
        .expect("experimental_study_summary registered");
    assert_eq!(summary_unwired.traits().risk, ToolRisk::ReadOnly);
    let err = summary_unwired
        .execute(
            serde_json::json!({ "instrument": "NSE:NIFTY50" }),
            dummy_tool_context(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Execution(..)));

    // 2. Wired StudyReader returns exact Decimal study summary + Markov probs + strike levels.
    let mut wired_reg = ToolRegistry::new();
    register_read_only_tools_with_studies(
        &mut wired_reg,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader {
            fail_content: false,
        }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
        Some(Arc::new(MockStudyReader { fail: false })),
    );
    let summary_tool = wired_reg.get("experimental_study_summary").unwrap();
    let out = summary_tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:NIFTY50",
                "asset_class": "index",
                "period": "weekly_expiry"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(out.ok);
    assert_eq!(out.payload["total_periods"], 90);
    assert_eq!(out.payload["strongest_next_markov_state"], "ShortNegative");

    // 3. historical_outlier_news expands [window_start - 2d, window_end + 1d] and retrieves
    //    both news and macro calendar events cleanly (and caches identical lookups).
    let today = Utc::now().date_naive();
    let w_start = (today - chrono::Duration::days(1)).to_string();
    let w_end = today.to_string();
    let news_tool = wired_reg
        .get("historical_outlier_news")
        .expect("historical_outlier_news registered");
    assert_eq!(news_tool.traits().risk, ToolRisk::ReadOnly);
    let news_out = news_tool
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "asset_class": "fno_stock",
                "window_start": w_start,
                "window_end": w_end,
                "keywords": "results rbi"
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert!(news_out.ok);
    assert_eq!(news_out.payload["news_archive_available"], true);
    assert_eq!(news_out.payload["macro_calendar_available"], true);
    let items = news_out.payload["items"].as_array().unwrap();
    assert!(
        items.len() >= 2,
        "expected news + RBI macro event in expanded window"
    );

    // 4. When news content archive is offline, macro calendar still returns honestly without panic.
    let mut offline_news_reg = ToolRegistry::new();
    register_read_only_tools_with_studies(
        &mut offline_news_reg,
        Arc::new(MockPortfolioReader),
        Arc::new(MockMarketReader { fail_quotes: false }),
        Arc::new(MockContentReader { fail_content: true }),
        Arc::new(MockCalendarReader),
        Arc::new(MockFundamentalReader { fail: false }),
        Arc::new(MockFundReader { fail: false }),
        Arc::new(MockIpoReader { fail: false }),
        Arc::new(MockTaxReportReader { fail: false }),
        None,
        None,
    );
    let offline_out = offline_news_reg
        .get("historical_outlier_news")
        .unwrap()
        .execute(
            serde_json::json!({
                "instrument": "NSE:RELIANCE",
                "window_start": w_start,
                "window_end": w_end
            }),
            dummy_tool_context(),
        )
        .await
        .unwrap();
    assert_eq!(offline_out.payload["news_archive_available"], false);
    assert_eq!(offline_out.payload["macro_calendar_available"], true);
}
