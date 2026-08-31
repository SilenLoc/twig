//! The namespace-level issue tracker.
//!
//! Tickets live in a git repository — one per namespace, at
//! `/{namespace}/ticket` — as readable TOML files. Both the web UI and ordinary
//! `git push` write to it, and the ingest pipeline merges concurrent writes
//! field by field so users never see a conflict.

pub mod attachment;
pub mod ingest;
pub mod markdown;
pub mod merge;
pub mod model;
pub mod ops;
pub mod pktline;
pub mod repo;
pub mod store;
