//! Error types for the MCP quantitative server.

/// MCP-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// JSON parse error
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Method not found
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid arguments
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// Backtest execution failed
    #[error("Backtest execution failed: {0}")]
    BacktestFailed(String),

    /// Strategy validation failed
    #[error("Strategy validation failed: {0}")]
    ValidationFailed(String),

    /// Indicator catalog error
    #[error("Indicator catalog error: {0}")]
    IndicatorError(String),

    /// Indicator library not configured or unreachable
    #[error("Library unavailable: {0}")]
    Unavailable(String),

    /// Indicator library is busy
    #[error("Library busy: {0}")]
    Busy(String),

    /// Indicator library call timed out
    #[error("Library timeout: {0}")]
    Timeout(String),

    /// Library entry not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Upstream library error
    #[error("Upstream library error: {0}")]
    Upstream(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Transport error
    #[error("Transport error: {0}")]
    Transport(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<tantra_agent_core::library::LibraryPortError> for McpError {
    fn from(e: tantra_agent_core::library::LibraryPortError) -> Self {
        use tantra_agent_core::library::LibraryPortErrorKind as K;
        match e.kind {
            K::Unavailable => McpError::Unavailable(e.message),
            K::InvalidRequest => McpError::InvalidArguments(e.message),
            K::NotFound => McpError::NotFound(e.message),
            K::Busy => McpError::Busy(e.message),
            K::Timeout => McpError::Timeout(e.message),
            K::Upstream => McpError::Upstream(e.message),
        }
    }
}

impl From<tantra_agent_core::strategy_loop::StrategyLoopPortError> for McpError {
    fn from(e: tantra_agent_core::strategy_loop::StrategyLoopPortError) -> Self {
        use tantra_agent_core::strategy_loop::StrategyLoopPortErrorKind as K;
        match e.kind {
            K::Unavailable => McpError::Unavailable(e.message),
            K::InvalidRequest => McpError::InvalidArguments(e.message),
            K::NotFound => McpError::NotFound(e.message),
            K::Busy => McpError::Busy(e.message),
            K::Timeout => McpError::Timeout(e.message),
            K::Upstream => McpError::Upstream(e.message),
        }
    }
}
