//! PII detection + substitution.
//!
//! Scans clipboard text for SSN, credit/debit card (Luhn-validated), bank
//! account, ABA routing, email, phone, and DOB. Returns byte-offset matches
//! that callers feed back into `substitute()` along with an optional
//! `PiiSubject` (test user) — the substitutor swaps each match for the
//! subject's matching field, formatted to mirror the original's separators.
//!
//! No I/O, no state. Regex patterns are compiled once via `Lazy`. The
//! `regex` crate has no lookarounds, so context heuristics (require keyword
//! within N chars) are done as post-match checks on the surrounding window.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Categories the detector understands. Numeric ordering also serves as
/// overlap-resolution priority (lower = wins). E.g. when a 9-digit run looks
/// like both SSN and Routing, Ssn (priority 0) wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PiiKind {
    Ssn,
    Card,
    Routing,
    Account,
    Dob,
    Email,
    Phone,
}

impl PiiKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssn => "ssn",
            Self::Card => "card",
            Self::Routing => "routing",
            Self::Account => "account",
            Self::Dob => "dob",
            Self::Email => "email",
            Self::Phone => "phone",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Ssn => 0,
            Self::Card => 1,
            Self::Routing => 2,
            Self::Account => 3,
            Self::Dob => 4,
            Self::Email => 5,
            Self::Phone => 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    pub kind: PiiKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    pub detections: Vec<Detection>,
}

impl ScanResult {
    pub fn is_sensitive(&self) -> bool {
        !self.detections.is_empty()
    }

    pub fn kinds(&self) -> Vec<PiiKind> {
        let mut out: Vec<PiiKind> = Vec::new();
        for d in &self.detections {
            if !out.contains(&d.kind) {
                out.push(d.kind);
            }
        }
        out
    }
}

/// Trait implemented by `state::test_users::TestUser` so this module stays
/// dependency-free. `card(nth)` returns the nth distinct card on the test
/// user's record so multiple card matches in the same source string get
/// mapped to multiple test cards in round-robin fashion.
pub trait PiiSubject {
    fn ssn(&self) -> Option<&str>;
    fn dob(&self) -> Option<&str>;
    fn email(&self) -> Option<&str>;
    fn phone(&self) -> Option<&str>;
    fn account(&self) -> Option<&str>;
    fn routing(&self) -> Option<&str>;
    fn card(&self, nth: usize) -> Option<&str>;
}

// --- Patterns ---

static RE_SSN_DASHED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());
static RE_NINE_DIGITS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{9}\b").unwrap());
static RE_CARD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").unwrap());
static RE_LONG_DIGITS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{8,17}\b").unwrap());
static RE_EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap());
static RE_PHONE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
        (?:\+?1[\s\-.]?)?            # optional country code
        (?:
            \(\d{3}\)\s*\d{3}[\-.\s]?\d{4}   # (555) 123-4567
            |
            \d{3}[\-.\s]\d{3}[\-.\s]\d{4}    # 555-123-4567 or 555.123.4567 or 555 123 4567
        )
        ",
    )
    .unwrap()
});
static RE_DOB_US: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12]\d|3[01])[/\-](?:19|20)\d{2}\b").unwrap()
});
static RE_DOB_ISO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:19|20)\d{2}-(?:0?[1-9]|1[0-2])-(?:0?[1-9]|[12]\d|3[01])\b").unwrap());

// --- Public scan entry point ---

pub fn scan(text: &str) -> ScanResult {
    if text.is_empty() {
        return ScanResult::default();
    }

    let mut raw: Vec<Detection> = Vec::new();

    raw.extend(detect_ssn(text));
    raw.extend(detect_card(text));
    raw.extend(detect_routing(text));
    raw.extend(detect_account(text));
    raw.extend(detect_dob(text));
    raw.extend(detect_email(text));
    raw.extend(detect_phone(text));

    ScanResult {
        detections: resolve_overlaps(raw),
    }
}

// --- Per-kind detectors ---

fn detect_ssn(text: &str) -> Vec<Detection> {
    let mut out: Vec<Detection> = Vec::new();
    for m in RE_SSN_DASHED.find_iter(text) {
        out.push(Detection {
            kind: PiiKind::Ssn,
            start: m.start(),
            end: m.end(),
        });
    }
    for m in RE_NINE_DIGITS.find_iter(text) {
        if has_keyword_nearby(text, m.start(), m.end(), &["ssn", "social", "ss#"], 30) {
            out.push(Detection {
                kind: PiiKind::Ssn,
                start: m.start(),
                end: m.end(),
            });
        }
    }
    out
}

fn detect_card(text: &str) -> Vec<Detection> {
    let mut out: Vec<Detection> = Vec::new();
    for m in RE_CARD.find_iter(text) {
        let raw = &text[m.start()..m.end()];
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 13 || digits.len() > 19 {
            continue;
        }
        if !luhn_valid(&digits) {
            continue;
        }
        out.push(Detection {
            kind: PiiKind::Card,
            start: m.start(),
            end: m.end(),
        });
    }
    out
}

fn detect_routing(text: &str) -> Vec<Detection> {
    let mut out: Vec<Detection> = Vec::new();
    for m in RE_NINE_DIGITS.find_iter(text) {
        let digits = &text[m.start()..m.end()];
        if !aba_valid(digits) {
            continue;
        }
        if !has_keyword_nearby(text, m.start(), m.end(), &["routing", "aba", "rtn"], 30) {
            continue;
        }
        out.push(Detection {
            kind: PiiKind::Routing,
            start: m.start(),
            end: m.end(),
        });
    }
    out
}

fn detect_account(text: &str) -> Vec<Detection> {
    let mut out: Vec<Detection> = Vec::new();
    for m in RE_LONG_DIGITS.find_iter(text) {
        if !has_keyword_nearby(text, m.start(), m.end(), &["account", "acct", "a/c"], 30) {
            continue;
        }
        out.push(Detection {
            kind: PiiKind::Account,
            start: m.start(),
            end: m.end(),
        });
    }
    out
}

fn detect_email(text: &str) -> Vec<Detection> {
    RE_EMAIL
        .find_iter(text)
        .map(|m| Detection {
            kind: PiiKind::Email,
            start: m.start(),
            end: m.end(),
        })
        .collect()
}

fn detect_phone(text: &str) -> Vec<Detection> {
    RE_PHONE
        .find_iter(text)
        .map(|m| Detection {
            kind: PiiKind::Phone,
            start: m.start(),
            end: m.end(),
        })
        .collect()
}

fn detect_dob(text: &str) -> Vec<Detection> {
    let mut out: Vec<Detection> = Vec::new();
    for re in [&*RE_DOB_US, &*RE_DOB_ISO] {
        for m in re.find_iter(text) {
            if !has_keyword_nearby(text, m.start(), m.end(), &["dob", "birth", "d.o.b"], 30) {
                continue;
            }
            out.push(Detection {
                kind: PiiKind::Dob,
                start: m.start(),
                end: m.end(),
            });
        }
    }
    out
}

// --- Helpers ---

fn has_keyword_nearby(
    text: &str,
    start: usize,
    end: usize,
    keywords: &[&str],
    window: usize,
) -> bool {
    let lo = start.saturating_sub(window);
    let hi = (end + window).min(text.len());
    let slice = match text.get(lo..hi) {
        Some(s) => s,
        None => return false,
    };
    let lc = slice.to_ascii_lowercase();
    keywords.iter().any(|k| lc.contains(*k))
}

fn luhn_valid(digits: &str) -> bool {
    let bytes = digits.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    let mut dbl = false;
    for &b in bytes.iter().rev() {
        if !b.is_ascii_digit() {
            return false;
        }
        let mut d = (b - b'0') as u32;
        if dbl {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        dbl = !dbl;
    }
    sum % 10 == 0
}

fn aba_valid(digits: &str) -> bool {
    if digits.len() != 9 {
        return false;
    }
    let d: Vec<u32> = digits
        .as_bytes()
        .iter()
        .map(|b| (b - b'0') as u32)
        .collect();
    let sum = 3 * (d[0] + d[3] + d[6]) + 7 * (d[1] + d[4] + d[7]) + (d[2] + d[5] + d[8]);
    sum % 10 == 0 && sum > 0
}

fn resolve_overlaps(mut all: Vec<Detection>) -> Vec<Detection> {
    if all.is_empty() {
        return all;
    }
    all.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.kind.priority().cmp(&b.kind.priority()))
    });
    let mut out: Vec<Detection> = Vec::with_capacity(all.len());
    for cand in all {
        match out.last() {
            Some(prev) if prev.end > cand.start => {
                if cand.kind.priority() < prev.kind.priority() {
                    out.pop();
                    out.push(cand);
                }
            }
            _ => out.push(cand),
        }
    }
    out
}

// --- Substitution ---

pub fn substitute<S: PiiSubject>(
    text: &str,
    detections: &[Detection],
    subject: Option<&S>,
) -> String {
    if detections.is_empty() {
        return text.to_string();
    }
    let mut sorted: Vec<&Detection> = detections.iter().collect();
    sorted.sort_by_key(|d| d.start);

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut card_nth: usize = 0;
    for d in sorted {
        if d.start < cursor || d.end > text.len() || d.end < d.start {
            continue;
        }
        out.push_str(&text[cursor..d.start]);
        let original = &text[d.start..d.end];
        let replacement = match d.kind {
            PiiKind::Ssn => substitute_digits(original, subject.and_then(|s| s.ssn()), mask_keep_last4),
            PiiKind::Card => {
                let nth = card_nth;
                card_nth += 1;
                substitute_digits(
                    original,
                    subject.and_then(|s| s.card(nth)),
                    mask_keep_last4,
                )
            }
            PiiKind::Routing => substitute_digits(
                original,
                subject.and_then(|s| s.routing()),
                mask_keep_last4,
            ),
            PiiKind::Account => substitute_digits(
                original,
                subject.and_then(|s| s.account()),
                mask_keep_last4,
            ),
            PiiKind::Phone => substitute_digits(
                original,
                subject.and_then(|s| s.phone()),
                mask_keep_last4,
            ),
            PiiKind::Dob => subject
                .and_then(|s| s.dob())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string())
                .unwrap_or_else(|| mask_dob(original)),
            PiiKind::Email => subject
                .and_then(|s| s.email())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string())
                .unwrap_or_else(|| mask_email(original)),
        };
        out.push_str(&replacement);
        cursor = d.end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Substitute a numeric PII match while preserving the original's separators.
/// Strips non-digits from the replacement, then walks the original char-by-char
/// emitting digits from the replacement for digit slots and the original
/// separator otherwise. Falls back to `mask_fn(original)` if the replacement
/// is None or has zero digits.
fn substitute_digits(
    original: &str,
    replacement: Option<&str>,
    mask_fn: fn(&str) -> String,
) -> String {
    let Some(rep) = replacement.filter(|s| !s.is_empty()) else {
        return mask_fn(original);
    };
    let digits: Vec<char> = rep.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return mask_fn(original);
    }
    let mut out = String::with_capacity(original.len());
    let mut di = 0usize;
    for c in original.chars() {
        if c.is_ascii_digit() {
            out.push(if di < digits.len() {
                digits[di]
            } else {
                // Replacement ran out of digits; cycle from the start so the
                // output is still all-digit-looking instead of "0"-padded.
                digits[di % digits.len()]
            });
            di += 1;
        } else {
            out.push(c);
        }
    }
    out
}

/// Mask preserving separators: replace every digit except the last four with
/// '*'. E.g. "123-45-6789" -> "***-**-6789", "4111 1111 1111 1111" ->
/// "**** **** **** 1111".
fn mask_keep_last4(original: &str) -> String {
    let total_digits = original.chars().filter(|c| c.is_ascii_digit()).count();
    let keep_after = total_digits.saturating_sub(4);
    let mut out = String::with_capacity(original.len());
    let mut seen = 0usize;
    for c in original.chars() {
        if c.is_ascii_digit() {
            if seen < keep_after {
                out.push('*');
            } else {
                out.push(c);
            }
            seen += 1;
        } else {
            out.push(c);
        }
    }
    out
}

fn mask_dob(_original: &str) -> String {
    // DOB has no "last 4" worth keeping — the year IS the sensitive part.
    // Full mask preserving separators is fine.
    let mut out = String::new();
    for c in _original.chars() {
        if c.is_ascii_digit() {
            out.push('*');
        } else {
            out.push(c);
        }
    }
    out
}

fn mask_email(original: &str) -> String {
    let mut parts = original.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if domain.is_empty() {
        return "*".repeat(original.chars().count());
    }
    let first = local.chars().next();
    let masked_local = match first {
        Some(c) => format!("{c}****"),
        None => "*****".to_string(),
    };
    format!("{masked_local}@{domain}")
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSubject {
        ssn: Option<String>,
        dob: Option<String>,
        email: Option<String>,
        phone: Option<String>,
        account: Option<String>,
        routing: Option<String>,
        cards: Vec<String>,
    }

    impl PiiSubject for FakeSubject {
        fn ssn(&self) -> Option<&str> {
            self.ssn.as_deref()
        }
        fn dob(&self) -> Option<&str> {
            self.dob.as_deref()
        }
        fn email(&self) -> Option<&str> {
            self.email.as_deref()
        }
        fn phone(&self) -> Option<&str> {
            self.phone.as_deref()
        }
        fn account(&self) -> Option<&str> {
            self.account.as_deref()
        }
        fn routing(&self) -> Option<&str> {
            self.routing.as_deref()
        }
        fn card(&self, nth: usize) -> Option<&str> {
            if self.cards.is_empty() {
                None
            } else {
                Some(&self.cards[nth % self.cards.len()])
            }
        }
    }

    fn alice() -> FakeSubject {
        FakeSubject {
            ssn: Some("900-11-1111".into()),
            dob: Some("1990-01-01".into()),
            email: Some("alice@test.fnba".into()),
            phone: Some("555-000-1111".into()),
            account: Some("0000111122223333".into()),
            routing: Some("021000021".into()), // valid ABA
            cards: vec!["4242424242424242".into(), "5555555555554444".into()],
        }
    }

    #[test]
    fn ssn_dashed_detected() {
        let res = scan("My SSN is 123-45-6789 and I want pizza.");
        assert_eq!(res.detections.len(), 1);
        assert_eq!(res.detections[0].kind, PiiKind::Ssn);
    }

    #[test]
    fn ssn_undashed_needs_keyword() {
        let no_ctx = scan("Random 9 digits: 123456789 ok");
        assert!(no_ctx.detections.is_empty(), "should not detect without keyword");

        let with_ctx = scan("SSN: 123456789");
        assert!(
            with_ctx.detections.iter().any(|d| d.kind == PiiKind::Ssn),
            "should detect 9 digits when 'SSN' is nearby"
        );
    }

    #[test]
    fn luhn_filters_card_false_positives() {
        let real = scan("Card: 4111 1111 1111 1111"); // valid Luhn
        assert!(real.detections.iter().any(|d| d.kind == PiiKind::Card));

        let fake = scan("Card: 1234 5678 9012 3456"); // invalid Luhn
        assert!(!fake.detections.iter().any(|d| d.kind == PiiKind::Card));
    }

    #[test]
    fn routing_requires_aba_and_keyword() {
        let valid = scan("Routing: 021000021"); // BoNY, valid ABA
        assert!(valid.detections.iter().any(|d| d.kind == PiiKind::Routing));

        let no_keyword = scan("021000021 alone");
        assert!(!no_keyword.detections.iter().any(|d| d.kind == PiiKind::Routing));

        let bad_aba = scan("Routing: 123456789"); // not a valid ABA checksum
        assert!(!bad_aba.detections.iter().any(|d| d.kind == PiiKind::Routing));
    }

    #[test]
    fn account_requires_keyword() {
        let no_kw = scan("12345678901234");
        assert!(!no_kw.detections.iter().any(|d| d.kind == PiiKind::Account));

        let kw = scan("Account 12345678901234");
        assert!(kw.detections.iter().any(|d| d.kind == PiiKind::Account));
    }

    #[test]
    fn email_phone_dob_basic() {
        let res = scan("Reach me at jane@example.com or (555) 123-4567. DOB 01/05/1990.");
        let kinds: Vec<_> = res.detections.iter().map(|d| d.kind).collect();
        assert!(kinds.contains(&PiiKind::Email));
        assert!(kinds.contains(&PiiKind::Phone));
        assert!(kinds.contains(&PiiKind::Dob));
    }

    #[test]
    fn dob_alone_is_not_flagged() {
        let res = scan("Meeting on 01/05/1990 at 3pm");
        assert!(!res.detections.iter().any(|d| d.kind == PiiKind::Dob));
    }

    #[test]
    fn substitute_with_test_user_preserves_separators() {
        let text = "SSN 123-45-6789";
        let res = scan(text);
        let user = alice();
        let out = substitute(text, &res.detections, Some(&user));
        assert_eq!(out, "SSN 900-11-1111");
    }

    #[test]
    fn substitute_without_test_user_masks_with_last4() {
        let text = "SSN 123-45-6789, card 4111 1111 1111 1111";
        let res = scan(text);
        let out: String = substitute::<FakeSubject>(text, &res.detections, None);
        assert!(out.contains("***-**-6789"), "got: {out}");
        assert!(out.contains("**** **** **** 1111"), "got: {out}");
    }

    #[test]
    fn substitute_round_robins_cards() {
        let text = "Cards 4111 1111 1111 1111 and 5555 5555 5555 4444";
        let res = scan(text);
        let user = alice();
        let out = substitute(text, &res.detections, Some(&user));
        // First card slot -> first test card; second -> second test card.
        assert!(out.contains("4242 4242 4242 4242"), "got: {out}");
        assert!(out.contains("5555 5555 5555 4444"), "got: {out}"); // second test card happens to share digits with the second source
    }

    #[test]
    fn ssn_and_phone_overlap_resolves_to_ssn() {
        // The 9-digit SSN context could in theory look phone-like; ensure Ssn wins.
        let text = "SSN 123-45-6789";
        let res = scan(text);
        let ssns = res
            .detections
            .iter()
            .filter(|d| d.kind == PiiKind::Ssn)
            .count();
        assert_eq!(ssns, 1);
    }

    #[test]
    fn email_mask_keeps_first_char_and_domain() {
        assert_eq!(mask_email("kevin.biesbrock@fnba.com"), "k****@fnba.com");
    }

    #[test]
    fn empty_input_is_inert() {
        let res = scan("");
        assert!(res.detections.is_empty());
        assert_eq!(substitute::<FakeSubject>("", &[], None), "");
    }
}
