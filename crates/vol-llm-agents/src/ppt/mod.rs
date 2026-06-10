//! PPT Agent: AI-powered PowerPoint generation.

pub mod agent;
pub mod analysis;
pub mod config;
pub mod prompts;
pub mod renderer;
pub mod template;
pub mod tools;
pub mod types;

pub use agent::{PptAgent, PptAgentError};
pub use config::PptAgentConfig;
pub use template::{PptTemplate, TemplateRegistry};
pub use types::{
    Outline, PptInput, PptOutput, Slide, SlideDef, SlideLayout, SlideType, StructuredRequirement,
};
