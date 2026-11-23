//! CoWriting's core: the AI log, the guard, the stats and the disclosure, with no window and no
//! network. Most of it is ported from the CoWriting plugin for Claude Code; each module names
//! the plugin file it follows, and `fixtures/goldens` holds the plugin's own output to compare with.

pub mod backend;
pub mod contract;
pub mod disclosure;
pub mod guard;
pub mod host;
pub mod log;
pub mod personas;
pub mod sidecar;
pub mod stats;
pub mod text;
pub mod thread;
pub mod tools;
pub mod turn;
pub mod types;
pub mod words;
