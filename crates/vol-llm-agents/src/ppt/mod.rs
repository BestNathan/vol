//! PPT Agent: AI-powered PowerPoint generation.

pub mod agent;
pub mod config;
pub mod types;
pub mod template;
pub mod renderer;
pub mod prompts;
pub mod tools;
pub mod analysis;

pub use agent::{PptAgent, PptAgentError};
pub use config::PptAgentConfig;
pub use types::{PptInput, PptOutput, Slide, SlideLayout, SlideType, StructuredRequirement, Outline, SlideDef};
pub use template::{TemplateRegistry, PptTemplate, TemplateLayout, LayoutElement, Position, ElementStyle, LayoutType, TemplateTags, ColorScheme, Typography};
