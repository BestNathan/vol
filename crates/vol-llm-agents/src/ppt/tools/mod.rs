//! PPT Agent 工具集。

pub mod content;
pub mod outline;
pub mod renderer;
pub mod template;

pub use content::{ContentError, ContentGeneratorTool};
pub use outline::{OutlineError, OutlineGeneratorTool};
pub use renderer::PptxRendererTool;
pub use template::TemplateMatcherTool;
