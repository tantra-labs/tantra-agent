//! ACP (Agent Client Protocol) server and client for the Tantra chat agent.

pub mod agent;
pub mod catalog;
pub mod client;
pub mod error;
pub mod meta;

pub use agent::{AgentHost, serve};
pub use catalog::{CatalogError, ModelCatalog, ModelInfo, StaticCatalog};
pub use client::{
    AcpClient, ClientEvent, ClientOptions, connect_embedded, connect_stdio, connect_ws,
};
pub use error::AcpError;
