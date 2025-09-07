use codex_core::protocol_config_types::ReasoningEffort;

/// A simple preset pairing a model slug with a reasoning effort.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Reasoning effort to apply for this preset.
    pub effort: ReasoningEffort,
}

/// Built-in list of model presets that pair a model with a reasoning effort.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_model_presets() -> &'static [ModelPreset] {
    // Order reflects effort from minimal to high.
    const PRESETS: &[ModelPreset] = &[
        ModelPreset {
            id: "gpt-5-minimal",
            label: "gpt-5 minimal",
            description: "— fastest responses with limited reasoning; ideal for coding, instructions, or lightweight tasks",
            model: "gpt-5",
            effort: ReasoningEffort::Minimal,
        },
        ModelPreset {
            id: "gpt-5-low",
            label: "gpt-5 low",
            description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
            model: "gpt-5",
            effort: ReasoningEffort::Low,
        },
        ModelPreset {
            id: "gpt-5-medium",
            label: "gpt-5 medium",
            description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
            model: "gpt-5",
            effort: ReasoningEffort::Medium,
        },
        ModelPreset {
            id: "gpt-5-high",
            label: "gpt-5 high",
            description: "— maximizes reasoning depth for complex or ambiguous problems",
            model: "gpt-5",
            effort: ReasoningEffort::High,
        },
        // xAI Grok 4 presets
        ModelPreset {
            id: "grok-4-minimal",
            label: "grok-4 minimal",
            description: "— fast responses; good for quick iterations",
            model: "grok-4",
            effort: ReasoningEffort::Minimal,
        },
        ModelPreset {
            id: "grok-4-medium",
            label: "grok-4 medium",
            description: "— balanced depth and speed",
            model: "grok-4",
            effort: ReasoningEffort::Medium,
        },
        ModelPreset {
            id: "grok-4-high",
            label: "grok-4 high",
            description: "— deeper reasoning for complex tasks",
            model: "grok-4",
            effort: ReasoningEffort::High,
        },
        // Claude presets
        ModelPreset {
            id: "claude-3-5-sonnet-minimal",
            label: "claude-3.5-sonnet minimal",
            description: "— speedy replies using Claude 3.5 Sonnet",
            model: "claude-3-5-sonnet",
            effort: ReasoningEffort::Minimal,
        },
        ModelPreset {
            id: "claude-3-5-sonnet-medium",
            label: "claude-3.5-sonnet medium",
            description: "— balanced for general tasks",
            model: "claude-3-5-sonnet",
            effort: ReasoningEffort::Medium,
        },
        ModelPreset {
            id: "claude-3-5-sonnet-high",
            label: "claude-3.5-sonnet high",
            description: "— deeper reasoning for complex problems",
            model: "claude-3-5-sonnet",
            effort: ReasoningEffort::High,
        },
        // Google Gemini presets
        ModelPreset {
            id: "gemini-1-5-pro-minimal",
            label: "gemini-1.5-pro minimal",
            description: "— fast responses with Gemini 1.5 Pro",
            model: "gemini-1.5-pro",
            effort: ReasoningEffort::Minimal,
        },
        ModelPreset {
            id: "gemini-1-5-pro-medium",
            label: "gemini-1.5-pro medium",
            description: "— balanced depth and speed",
            model: "gemini-1.5-pro",
            effort: ReasoningEffort::Medium,
        },
        ModelPreset {
            id: "gemini-1-5-pro-high",
            label: "gemini-1.5-pro high",
            description: "— deeper reasoning for complex tasks",
            model: "gemini-1.5-pro",
            effort: ReasoningEffort::High,
        },
    ];
    PRESETS
}
