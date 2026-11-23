//! CoWriting's core: the AI log, the guard, the stats and the disclosure, with no window and no
//! network. Most of it is ported from the CoWriting plugin for Claude Code; each module names
//! the plugin file it follows, and `fixtures/goldens` holds the plugin's own output to compare with.

pub mod actions;
pub mod backend;
pub mod briefs;
pub mod contract;
pub mod disclosure;
pub mod guard;
pub mod host;
pub mod log;
pub mod paste_tracking;
pub mod personas;
pub mod review;
pub mod scope;
pub mod sidecar;
pub mod sources;
pub mod stats;
pub mod text;
pub mod thread;
pub mod tools;
pub mod turn;
pub mod types;
pub mod watch;
pub mod words;
