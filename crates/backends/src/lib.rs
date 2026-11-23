//! How the editor reaches a model. Each backend makes one model call; the core's turn runs the
//! tools and writes the log, so guard and log are the same whichever backend answers.

pub mod anthropic;
pub mod claude_code;
pub mod fake;
pub mod lookup;
pub mod openai;
