//! The models Umbra knows by name, offered in the configuration modal's model lists. Any other
//! model can still be typed as a custom model.

/// One known model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownModel {
    pub label: &'static str,
    /// The provider kind that serves it: `anthropic` or `openai`.
    pub kind: &'static str,
    /// The model id sent to the API.
    pub model: &'static str,
    pub context_window: u64,
}

/// The known models: the Claude models, then the OpenAI models in the built in context window
/// table.
pub const KNOWN_MODELS: &[KnownModel] = &[
    KnownModel { label: "Fable 5.2", kind: "anthropic", model: "claude-fable-5-2", context_window: 200_000 },
    KnownModel { label: "Opus 5.5", kind: "anthropic", model: "claude-opus-5-5", context_window: 200_000 },
    KnownModel { label: "Sonnet 5.5", kind: "anthropic", model: "claude-sonnet-5-5", context_window: 200_000 },
    KnownModel { label: "Haiku 5.5", kind: "anthropic", model: "claude-haiku-5-5", context_window: 200_000 },
    KnownModel { label: "GPT-5", kind: "openai", model: "gpt-5", context_window: 400_000 },
    KnownModel { label: "GPT-4.1", kind: "openai", model: "gpt-4.1", context_window: 1_047_576 },
    KnownModel { label: "GPT-4o", kind: "openai", model: "gpt-4o", context_window: 128_000 },
    KnownModel { label: "o3", kind: "openai", model: "o3", context_window: 200_000 },
];

/// The known models a provider kind serves.
/// @param kind - the provider kind
pub fn known_for_kind(kind: &str) -> Vec<&'static KnownModel> {
    KNOWN_MODELS.iter().filter(|m| m.kind == kind).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_model_has_the_window_the_built_in_table_gives() {
        for model in KNOWN_MODELS {
            assert_eq!(crate::utils::tokens::known_context_window(model.model), Some(model.context_window), "{}", model.model);
        }
        assert_eq!(known_for_kind("anthropic").len(), 4);
    }
}
