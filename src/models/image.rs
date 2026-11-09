//! An image sent to the model, already checked and scaled, with its bytes as base64 text.

use serde::{Deserialize, Serialize};

/// The visual tokens one 28x28 patch costs, the rule Anthropic documents and that Qwen's vision
/// encoder follows closely.
const PATCH: u32 = 28;

/// One image in a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    /// The file's bytes as standard base64.
    pub data: String,
    pub width: u32,
    pub height: u32,
    /// Where it came from: a file path, or `mcp:<server>.<tool>`.
    #[serde(default)]
    pub source: String,
}

impl ImageData {
    /// The image as a `data:` URL, the form OpenAI compatible servers take.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, self.data)
    }

    /// The tokens the image adds to a request: one per 28x28 patch.
    pub fn estimated_tokens(&self) -> u64 {
        u64::from(self.width.div_ceil(PATCH)) * u64::from(self.height.div_ceil(PATCH))
    }

    /// A short description such as `image/png 800x600 (120 KB)`.
    pub fn describe(&self) -> String {
        let kilobytes = (self.data.len() * 3 / 4).div_ceil(1024);
        format!("{} {}x{} ({kilobytes} KB)", self.media_type, self.width, self.height)
    }
}
