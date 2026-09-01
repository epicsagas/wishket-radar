//! Tech profile (profile.yaml) loading + deterministic keyword matching.
//!
//! Resolution order: $WISHKET_PROFILE > ${CLAUDE_PLUGIN_ROOT}/profile.yaml >
//! <exe>/../../../profile.yaml (release binary lives in server/target/release).
//! Re-read on every call so profile edits apply without a server restart.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct Skill {
    pub name: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 상대적 중요도 (기본 1)
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub headline: Option<String>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    pub score: u32,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
}

fn expand_tilde(p: &std::path::Path) -> PathBuf {
    if let Some(p_str) = p.to_str() {
        if let Some(stripped) = p_str.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(stripped);
            }
        }
    }
    p.to_path_buf()
}

pub fn profile_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("WISHKET_PROFILE") {
        let p = expand_tilde(&PathBuf::from(p));
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let p1 = home.join(".wishket").join("profile.yaml");
        if p1.is_file() {
            return Some(p1);
        }
        let p2 = home.join(".wishket-radar").join("profile.yaml");
        if p2.is_file() {
            return Some(p2);
        }
    }
    if let Ok(root) = std::env::var("CLAUDE_PLUGIN_ROOT") {
        let p = PathBuf::from(root).join("profile.yaml");
        if p.is_file() {
            return Some(p);
        }
    }
    // release binary: <root>/server/target/release/wishket-mcp
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent()?.to_path_buf();
        for _ in 0..3 {
            dir.pop();
        }
        let p = dir.join("profile.yaml");
        if p.is_file() {
            return Some(p);
        }
    }
    std::env::current_dir().ok().map(|d| d.join("profile.yaml"))
}

pub fn load() -> Result<Profile, String> {
    let Some(path) = profile_path() else {
        return Err("profile.yaml not found (set WISHKET_PROFILE)".into());
    };
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Deterministic keyword prefilter: 100 * matched_weight / total_weight.
/// Korean keyword matching is exact-substring, English case-insensitive.
pub fn score(profile: &Profile, text: &str) -> MatchResult {
    let hay_lower = text.to_lowercase();
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut matched_w = 0u64;
    let mut total_w = 0u64;
    for s in &profile.skills {
        total_w += s.weight as u64;
        let hit = if s.keywords.is_empty() {
            contains_ci(&hay_lower, &s.name)
        } else {
            s.keywords.iter().any(|k| contains_ci(&hay_lower, k))
        };
        if hit {
            matched_w += s.weight as u64;
            matched.push(s.name.clone());
        } else {
            missing.push(s.name.clone());
        }
    }
    let score = if total_w == 0 {
        0
    } else {
        ((matched_w * 100) / total_w) as u32
    };
    MatchResult { score, matched, missing }
}

fn contains_ci(hay_lower: &str, needle: &str) -> bool {
    if needle.chars().any(|c| c.is_ascii_alphabetic()) {
        hay_lower.contains(&needle.to_lowercase())
    } else {
        hay_lower.contains(needle)
    }
}

/// Match over a card: title + role + skills + location.
pub fn score_card(profile: &Profile, card_text: &str) -> Value {
    let m = score(profile, card_text);
    json!({ "score": m.score, "matched": m.matched, "missing": m.missing })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        serde_yaml::from_str(
            "name: test\nskills:\n  - name: Rust\n    keywords: [rust, 러스트]\n    weight: 3\n  - name: Flutter\n    keywords: [flutter]\n    weight: 1\n",
        )
        .unwrap()
    }

    #[test]
    fn scoring_weights() {
        let p = profile();
        // rust 히트(3/4), flutter 미스 → 75점
        let m = score(&p, "Rust 백엔드 개발자 채용");
        assert_eq!(m.score, 75);
        assert_eq!(m.matched, vec!["Rust"]);
        assert_eq!(m.missing, vec!["Flutter"]);
        // 둘 다 히트 → 100
        let m = score(&p, "flutter + rust 앱");
        assert_eq!(m.score, 100);
        // 대소문자 무시
        let m = score(&p, "RUST 전문가");
        assert_eq!(m.score, 75);
    }

    #[test]
    fn expand_tilde_test() {
        let p = expand_tilde(&PathBuf::from("~/test/path.yaml"));
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(p, PathBuf::from(home).join("test/path.yaml"));
        }
        let rel = expand_tilde(&PathBuf::from("relative/path.yaml"));
        assert_eq!(rel, PathBuf::from("relative/path.yaml"));
    }
}
