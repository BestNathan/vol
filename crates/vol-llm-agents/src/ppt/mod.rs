//! PPT Agent: AI-powered PowerPoint generation.

pub mod agent;
pub mod config;
pub mod types;
pub mod template;
pub mod renderer;
pub mod tools;
pub mod prompts;

pub use agent::PptAgent;
pub use config::PptAgentConfig;
pub use types::{PptInput, PptOutput, Slide, SlideLayout, SlideType, StructuredRequirement, Outline, SlideDef};
pub use template::{TemplateRegistry, PptTemplate};
