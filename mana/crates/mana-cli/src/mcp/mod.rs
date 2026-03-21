//! MCP (Model Context Protocol) server for units.
//!
//! Exposes units operations as MCP tools over stdio transport,
//! enabling integration with Cursor, Windsurf, Claude Desktop, Cline,
//! and any MCP-compatible client.

pub mod protocol;
pub mod resources;
pub mod server;
pub mod tools;
