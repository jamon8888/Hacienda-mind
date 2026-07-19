//! Presidio-style regex PII/secret detector. Catches structured PII and secrets
//! the GLiNER2 ML model misses (emails, phones, dates, URLs, cloud keys, JWT,
//! PEM). Returns the same `(start, end, category)` shape the candle model uses.

use once_cell::sync::Lazy;
use regex::Regex;

pub type RegexSpan = (usize, usize, &'static str);

static PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    let build = |re: &'static str| Regex::new(re).expect("valid pii regex");
    vec![
        ("email", build(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}")),
        ("phone", build(r"(?:\+?\d{1,3}[\s.\-]?)?(?:\(?\d{2,4}\)?[\s.\-]?){2,4}\d{2,4}")),
        ("date", build(r"\b\d{4}-\d{2}-\d{2}\b|\b\d{1,2}/\d{1,2}/\d{2,4}\b")),
        ("url", build(r#"https?://[^\s\"'<>]+"#)),
        ("aws_key", build(r"(?i)\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
        ("api_key", build(r"(?i)\b(?:sk|pk|rk)-[a-z0-9]{16,}\b|ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9_]{22,}")),
        ("jwt", build(r"eyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+")),
        ("pem", build(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")),
        ("env_assign", build(r#"(?i)\b(?:API_KEY|SECRET|TOKEN|PASSWORD|PASSWD|ACCESS_KEY)\s*=\s*['\"]?[^\s'"\"]{4,}"#)),
    ]
});

/// Scan `text` for structured PII/secret spans.
pub fn detect_regex_pii(text: &str) -> Vec<RegexSpan> {
    let mut out = Vec::new();
    for (category, re) in PATTERNS.iter() {
        for m in re.find_iter(text) {
            out.push((m.start(), m.end(), *category));
        }
    }
    out.sort_by_key(|s| s.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_email_and_aws_key() {
        let spans = detect_regex_pii("contact maria@acme.com or akiaiosfodnn7example");
        let cats: Vec<&str> = spans.iter().map(|s| s.2).collect();
        assert!(cats.contains(&"email"));
        assert!(cats.contains(&"aws_key"));
    }

    #[test]
    fn detects_sk_token_and_jwt() {
        let spans = detect_regex_pii("key=sk-1234567890abcdef and eyJhbGciOiJIUzI1Ni.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKx");
        let cats: Vec<&str> = spans.iter().map(|s| s.2).collect();
        assert!(cats.contains(&"api_key"));
        assert!(cats.contains(&"jwt"));
    }
}