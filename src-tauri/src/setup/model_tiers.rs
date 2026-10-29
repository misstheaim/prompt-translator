//! Model tier table (plan T34, D28) from `docs/spikes/local-model-eval.md` as data, and the
//! pure function that picks a tier for a machine.
//!
//! There are no CPU tiers (owner decision on #45): a machine without a usable GPU gets tier
//! `none`, no model, and Claude Code is the recommended provider. Ollama stays selectable.

use serde::Serialize;
use ts_rs::TS;

use super::environment::GpuInfo;

/// Dedicated VRAM bounds in MB. Cards report slightly less than their nominal size (a "6 GB"
/// card shows about 6100 MB, a "10 GB" one about 10200), so the bounds sit a little below the
/// nominal 6 GB and above the nominal 10 GB.
const MEDIUM_MIN_VRAM_MB: u64 = 5_800;
const LARGE_MIN_VRAM_MB: u64 = 10_500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ModelTier {
    GpuSmall,
    GpuMedium,
    GpuLarge,
    /// No usable GPU: no local model is recommended.
    None,
}

/// One row of the tier table.
#[derive(Debug, Clone, Copy)]
pub struct TierInfo {
    pub tier: ModelTier,
    /// `None` for [`ModelTier::None`].
    pub model: Option<&'static str>,
    pub download_mb: Option<u64>,
}

/// The tier table. `gpu-medium` was measured; the others are estimates (see the spike report).
pub const TIERS: [TierInfo; 4] = [
    TierInfo {
        tier: ModelTier::GpuSmall,
        model: Some("qwen3.5:4b"),
        download_mb: Some(3_300),
    },
    TierInfo {
        tier: ModelTier::GpuMedium,
        model: Some("qwen3.5:9b"),
        download_mb: Some(6_600),
    },
    TierInfo {
        tier: ModelTier::GpuLarge,
        model: Some("qwen3.5:9b"),
        download_mb: Some(6_600),
    },
    TierInfo {
        tier: ModelTier::None,
        model: None,
        download_mb: None,
    },
];

/// What `environment_check` recommends for the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recommendation {
    pub tier: ModelTier,
    /// Ollama model tag; `None` when the tier is `none` (use Claude Code).
    pub model: Option<String>,
    /// Download size in MB; `None` when the tier is `none`.
    #[ts(type = "number | null")]
    pub download_mb: Option<u64>,
}

/// The table row for a tier.
pub fn tier_info(tier: ModelTier) -> &'static TierInfo {
    TIERS
        .iter()
        .find(|t| t.tier == tier)
        .expect("every tier is in the table")
}

/// Picks the tier from the usable GPU with the most VRAM; no usable GPU means `none`
/// (integrated graphics and GPUs under 4 GB count as no GPU, see `environment::classify_gpu`).
pub fn recommend(gpus: &[GpuInfo]) -> Recommendation {
    let best = gpus.iter().filter(|g| g.usable).map(|g| g.vram_mb).max();
    let tier = match best {
        None => ModelTier::None,
        Some(v) if v >= LARGE_MIN_VRAM_MB => ModelTier::GpuLarge,
        Some(v) if v >= MEDIUM_MIN_VRAM_MB => ModelTier::GpuMedium,
        Some(_) => ModelTier::GpuSmall,
    };
    let info = tier_info(tier);
    Recommendation {
        tier,
        model: info.model.map(str::to_owned),
        download_mb: info.download_mb,
    }
}
