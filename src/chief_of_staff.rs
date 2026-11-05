//! Pillar 2: 24/7 "Chief of Staff" Agent (The Otto Replacement).
//!
//! Implements deterministic Inbox & Calendar triage (`Ignore`, `InfoOnly`, `ActionRequired`),
//! 3-bullet mobile summary cards with interactive `[Approve Send]` buttons for Telegram/Slack,
//! and residential browser automation workflows.

use serde::{Deserialize, Serialize};

/// Classification bucket assigned to incoming emails, messages, or calendar invites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageCategory {
    /// Automated marketing, newsletters, or noise requiring zero notification.
    Ignore,
    /// Receipts, shipping updates, or status notes included in the passive digest.
    InfoOnly,
    /// Direct questions, scheduling requests, or urgent items requiring a reply or action.
    ActionRequired,
}

/// Interactive button rendered on Telegram or Slack mobile notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobileActionButton {
    ApproveSend { draft_id: String },
    EditReply { draft_id: String },
    SnoozeOneHour { item_id: String },
    Archive { item_id: String },
}

/// Incoming email or message evaluated during the 15-minute triage cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub sender: String,
    pub subject: String,
    pub preview_text: String,
    pub is_bulk_newsletter: bool,
    pub asks_direct_question: bool,
    pub mentions_urgent_deadline: bool,
}

/// Structured 3-bullet Telegram/Slack card produced by the Chief of Staff agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileTriageCard {
    pub item_id: String,
    pub category: TriageCategory,
    pub three_bullet_summary: [String; 3],
    pub drafted_reply: Option<String>,
    pub buttons: Vec<MobileActionButton>,
}

/// Residential browser automation job running on the appliance's home Wi-Fi IP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResidentialBrowserTask {
    /// Track product or flight pricing on sites that block cloud datacenter IPs.
    TrackPrice {
        name: String,
        url: String,
        target_price_cents: u64,
    },
    /// Download monthly PDF statements or SaaS invoices into `./data/invoices`.
    DownloadMonthlyStatement {
        provider: String,
        portal_url: String,
    },
    /// Monitor competitor changelog or pricing page for DOM diffs.
    MonitorPageDiff {
        competitor: String,
        page_url: String,
    },
}

/// Classifies an [`InboxItem`] and builds a 3-bullet mobile triage card with `[Approve Send]` buttons
/// when a reply is warranted.
#[must_use = "triage card should be queued for Telegram/Slack delivery or archived"]
pub fn triage_inbox_item(item: &InboxItem) -> MobileTriageCard {
    let category = if item.is_bulk_newsletter && !item.mentions_urgent_deadline {
        TriageCategory::Ignore
    } else if item.asks_direct_question || item.mentions_urgent_deadline {
        TriageCategory::ActionRequired
    } else {
        TriageCategory::InfoOnly
    };

    let urgency_bullet = match category {
        TriageCategory::ActionRequired => {
            "Priority: Action Required (direct question or deadline detected)".to_string()
        }
        TriageCategory::InfoOnly => "Priority: Info-Only (no immediate reply needed)".to_string(),
        TriageCategory::Ignore => "Priority: Ignored (automated bulk/newsletter)".to_string(),
    };

    let three_bullet_summary = [
        format!("From: {} — {}", item.sender, item.subject),
        format!("Context: {}", item.preview_text),
        urgency_bullet,
    ];

    let draft_id = format!("draft-{}", item.id);
    let (drafted_reply, buttons) = match category {
        TriageCategory::ActionRequired => (
            Some(format!(
                "Hi {}, thanks for reaching out regarding \"{}\". I reviewed this and will follow up shortly.",
                item.sender, item.subject
            )),
            vec![
                MobileActionButton::ApproveSend {
                    draft_id: draft_id.clone(),
                },
                MobileActionButton::EditReply { draft_id },
                MobileActionButton::SnoozeOneHour {
                    item_id: item.id.clone(),
                },
            ],
        ),
        TriageCategory::InfoOnly => (
            None,
            vec![MobileActionButton::Archive {
                item_id: item.id.clone(),
            }],
        ),
        TriageCategory::Ignore => (None, Vec::new()),
    };

    MobileTriageCard {
        item_id: item.id.clone(),
        category,
        three_bullet_summary,
        drafted_reply,
        buttons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_urgent_question_as_action_required_with_approve_send_button() {
        let item = InboxItem {
            id: "msg-101".to_string(),
            sender: "alice@partner.io".to_string(),
            subject: "Contract sign-off before 5pm".to_string(),
            preview_text: "Can you confirm the revised MSA terms today?".to_string(),
            is_bulk_newsletter: false,
            asks_direct_question: true,
            mentions_urgent_deadline: true,
        };

        let card = triage_inbox_item(&item);
        assert_eq!(card.category, TriageCategory::ActionRequired);
        assert!(card.drafted_reply.is_some());
        assert_eq!(card.three_bullet_summary.len(), 3);
        assert!(card.buttons.contains(&MobileActionButton::ApproveSend {
            draft_id: "draft-msg-101".to_string()
        }));
    }

    #[test]
    fn classifies_bulk_newsletter_as_ignore_with_no_buttons() {
        let item = InboxItem {
            id: "msg-102".to_string(),
            sender: "digest@newsletter.example".to_string(),
            subject: "Weekly Promo Roundup".to_string(),
            preview_text: "Top 10 deals this week".to_string(),
            is_bulk_newsletter: true,
            asks_direct_question: false,
            mentions_urgent_deadline: false,
        };

        let card = triage_inbox_item(&item);
        assert_eq!(card.category, TriageCategory::Ignore);
        assert!(card.drafted_reply.is_none());
        assert!(card.buttons.is_empty());
    }
}
