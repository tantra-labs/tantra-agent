//! Typed errors for the ACP crate.

use crate::catalog::CatalogError;

#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("acp protocol error: {0}")]
    Protocol(String),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("acp connection closed")]
    Closed,
}

impl From<agent_client_protocol::Error> for AcpError {
    fn from(e: agent_client_protocol::Error) -> Self {
        AcpError::Protocol(e.to_string())
    }
}
