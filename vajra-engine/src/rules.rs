use std::path::PathBuf;

use regex::Regex;
use vajra_protocol::Priority;

use crate::download_task::DownloadRequest;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutomationRule {
    pub name: String,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RuleCondition {
    ExtensionEquals(String),
    DomainMatches(String),
    SizeGreaterThan(u64), // bytes
    FilenameRegex(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RuleAction {
    MoveToDirectory(PathBuf),
    RunScript(PathBuf),
    SetPriority(Priority),
    AddTag(String),
}

#[derive(Default, Debug, Clone)]
pub struct RulesEngine {
    pub rules: Vec<AutomationRule>,
}

impl RulesEngine {
    pub fn evaluate_and_apply(&self, req: &mut DownloadRequest) {
        for rule in &self.rules {
            if self.matches(rule, req) {
                self.apply_actions(&rule.actions, req);
            }
        }
    }

    fn matches(&self, rule: &AutomationRule, req: &DownloadRequest) -> bool {
        if rule.conditions.is_empty() {
            return false;
        }

        rule.conditions.iter().all(|cond| match cond {
            RuleCondition::ExtensionEquals(ext) => {
                req.url.ends_with(ext) || req.filename.as_ref().is_some_and(|f| f.ends_with(ext))
            }
            RuleCondition::DomainMatches(domain) => req.url.contains(domain),
            RuleCondition::SizeGreaterThan(_) => true, // Size is evaluated after metadata fetch
            RuleCondition::FilenameRegex(pattern) => {
                if let Ok(re) = Regex::new(pattern) {
                    req.filename.as_ref().is_some_and(|f| re.is_match(f))
                } else {
                    false
                }
            }
        })
    }

    fn apply_actions(&self, actions: &[RuleAction], req: &mut DownloadRequest) {
        for action in actions {
            match action {
                RuleAction::MoveToDirectory(dir) => {
                    req.dest_dir = dir.clone();
                }
                RuleAction::SetPriority(_prio) => {
                    // Assuming we might add priority to DownloadRequest later,
                    // or handle it in the queue directly.
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vajra_protocol::QueueType;

    use super::*;

    #[test]
    fn test_rule_extension_equals() {
        let rule = AutomationRule {
            name: "PDF rule".into(),
            conditions: vec![RuleCondition::ExtensionEquals(".pdf".into())],
            actions: vec![RuleAction::MoveToDirectory(PathBuf::from("/documents"))],
        };
        let engine = RulesEngine { rules: vec![rule] };

        let mut req = DownloadRequest {
            url: "http://example.com/file.pdf".into(),
            mirrors: vec![],
            dest_dir: PathBuf::from("/downloads"),
            filename: None,
            timeout_secs: None,
            connect_timeout_secs: None,
            max_connections: 4,
            speed_limit: 0,
            throttle: None,
            delete_on_failure: false,
            queue_type: QueueType::Standard,
            sync_interval_secs: 0,
            referrer: None,
            cookie_header: None,
            user_agent: None,
            authorization: None,
            proxy: None,
            proxies: vec![],
            local_address: None,
            use_ytdlp: false,
            ytdlp_format: None,
            ytdlp_subtitles: false,
            ytdlp_playlist: false,
            use_http3: false,
            expected_hash: None,
            auto_extract: false,
            post_processing_script: None,
            av_scan_path: None,
            av_scan_args: vec![],
            schedule_at: None,
            daemon_config: None,
            priority: vajra_protocol::Priority::Normal,
            tags: vec![],
            tcp_multiplexing_opt: false,
            adaptive_chunk_v2: false,
        };

        engine.evaluate_and_apply(&mut req);
        assert_eq!(req.dest_dir, PathBuf::from("/documents"));
    }
}
