//! Pillar 3: Free Local "Background AI" Microservices ($0 API Cost).
//!
//! Configures Apple Silicon unified-memory AI workloads:
//! 1. 4-bit 7B–8B local LLM (`qwen2.5:7b-instruct-q4_K_M` / `llama3.1:8b-instruct-q4_K_M`) at ~35–45 tok/s.
//! 2. Unlimited Local Whisper (`whisper-large-v3-turbo`) transcribing 1 hour of audio in ~90s.
//! 3. Always-On Local Embeddings (`nomic-embed-text`) for private LAN semantic search (RAG).
//! 4. High-Volume Scraper / Log Filter (scores 100% of raw items locally and escalates only the top 1% to cloud APIs).

use crate::hardware::MemoryBudget;
use serde::{Deserialize, Serialize};

/// Recommended local AI model stack tailored to the appliance's memory budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAiStackConfig {
    pub primary_llm_tag: String,
    pub primary_llm_ram_mb: u32,
    pub estimated_tokens_per_sec: u32,
    pub embedding_model_tag: String,
    pub embedding_ram_mb: u32,
    pub whisper_model_tag: String,
    pub whisper_watch_inbox_dir: String,
    pub whisper_transcript_out_dir: String,
    pub idle_keep_alive_secs: u32,
}

/// A raw scraped web page, lead, or log record evaluated by the local 8B classifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrapedCandidate {
    pub id: String,
    pub source_url: String,
    pub local_relevance_score: u8,
    pub extracted_json_summary: String,
}

/// Result of running the 99%-local / 1%-frontier scraper filter over a batch of candidates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScraperFilterReport {
    pub total_evaluated_locally: usize,
    pub discarded_locally: usize,
    pub escalated_to_frontier: Vec<ScrapedCandidate>,
    pub estimated_cloud_tokens_saved: u64,
}

impl LocalAiStackConfig {
    /// Selects an optimal local AI model stack based on the hardware [`MemoryBudget`].
    #[must_use = "AI stack configuration should be applied to gateway services or displayed"]
    pub fn from_memory_budget(budget: &MemoryBudget) -> Self {
        let (primary_llm_tag, primary_llm_ram_mb, estimated_tokens_per_sec) =
            if budget.can_run_local_llm {
                ("qwen2.5:7b-instruct-q4_K_M".to_string(), 5120, 40)
            } else {
                ("qwen2.5:1.5b-instruct-q4_K_M".to_string(), 1280, 25)
            };

        Self {
            primary_llm_tag,
            primary_llm_ram_mb,
            estimated_tokens_per_sec,
            embedding_model_tag: "nomic-embed-text:v1.5".to_string(),
            embedding_ram_mb: 384,
            whisper_model_tag: "whisper-large-v3-turbo-q5_0".to_string(),
            whisper_watch_inbox_dir: "./data/voice-inbox".to_string(),
            whisper_transcript_out_dir: "./data/transcripts".to_string(),
            idle_keep_alive_secs: budget.llm_idle_unload_secs,
        }
    }
}

/// Filters a high-volume stream of scraped items locally using the 8B model's relevance score
/// (`0..=100`) and escalates only candidates meeting `min_frontier_score` (default `95`) to
/// paid frontier cloud models.
#[must_use = "filter report contains the escalated frontier candidates and token savings"]
pub fn filter_scraped_batch(
    candidates: &[ScrapedCandidate],
    min_frontier_score: u8,
    avg_tokens_per_item: u64,
) -> ScraperFilterReport {
    let mut escalated_to_frontier = Vec::new();

    for item in candidates {
        if item.local_relevance_score >= min_frontier_score {
            escalated_to_frontier.push(item.clone());
        }
    }

    let total_evaluated_locally = candidates.len();
    let discarded_locally = total_evaluated_locally.saturating_sub(escalated_to_frontier.len());
    let estimated_cloud_tokens_saved =
        (discarded_locally as u64).saturating_mul(avg_tokens_per_item);

    ScraperFilterReport {
        total_evaluated_locally,
        discarded_locally,
        escalated_to_frontier,
        estimated_cloud_tokens_saved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{CpuArch, HardwareProfile};

    #[test]
    fn selects_qwen_7b_4bit_stack_for_16gb_m1_air() {
        let profile = HardwareProfile {
            model_name: "MacBookAir10,1".to_string(),
            arch: CpuArch::AppleSilicon,
            memory_gb: 16,
            has_battery: true,
            battery_health_percent: Some(90),
            fanless: true,
        };
        let budget = profile
            .compute_memory_budget()
            .unwrap_or_else(|_| unreachable!());
        let stack = LocalAiStackConfig::from_memory_budget(&budget);

        assert_eq!(stack.primary_llm_tag, "qwen2.5:7b-instruct-q4_K_M");
        assert_eq!(stack.primary_llm_ram_mb, 5120);
        assert_eq!(stack.embedding_model_tag, "nomic-embed-text:v1.5");
        assert_eq!(stack.idle_keep_alive_secs, 300);
    }

    #[test]
    fn filters_high_volume_batch_and_escalates_only_top_candidates() {
        let items = vec![
            ScrapedCandidate {
                id: "lead-1".to_string(),
                source_url: "https://example.com/1".to_string(),
                local_relevance_score: 42,
                extracted_json_summary: "{\"intent\":\"low\"}".to_string(),
            },
            ScrapedCandidate {
                id: "lead-2".to_string(),
                source_url: "https://example.com/2".to_string(),
                local_relevance_score: 81,
                extracted_json_summary: "{\"intent\":\"medium\"}".to_string(),
            },
            ScrapedCandidate {
                id: "lead-3".to_string(),
                source_url: "https://example.com/3".to_string(),
                local_relevance_score: 98,
                extracted_json_summary: "{\"intent\":\"enterprise_buyer\"}".to_string(),
            },
        ];

        let report = filter_scraped_batch(&items, 95, 1200);
        assert_eq!(report.total_evaluated_locally, 3);
        assert_eq!(report.discarded_locally, 2);
        assert_eq!(report.escalated_to_frontier.len(), 1);
        assert_eq!(report.escalated_to_frontier[0].id, "lead-3");
        assert_eq!(report.estimated_cloud_tokens_saved, 2400);
    }
}
