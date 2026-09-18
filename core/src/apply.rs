//! Strategien anwenden: aus Treffer + Regel wird ein Ersatz; aus Text + Ersetzungen
//! wird die Ausgabe.

use crate::dates;
use crate::dict::Dictionaries;
use crate::model::{Category, CategoryRule, NamePart, Strategy};
use crate::pseudo::Pseudonymizer;

/// Ersatztext für einen Treffer nach Strategie.
pub fn replacement(text: &str, category: Category, parts: &[NamePart], seps: &[String], rule: &CategoryRule, p: &mut Pseudonymizer, d: &Dictionaries) -> String {
    match rule.strategy {
        Strategy::Pseudonym => pseudonym(text, category, parts, seps, p, d),
        Strategy::Placeholder => {
            let n = p.placeholder_no(category, text);
            let tpl = if rule.placeholder.trim().is_empty() { "[{cat} {n}]" } else { rule.placeholder.as_str() };
            tpl.replace("{cat}", category.placeholder_word()).replace("{n}", &n.to_string())
        }
        Strategy::Redact => {
            let ch = rule.redact_char.chars().next().unwrap_or('█');
            let len = if rule.keep_length { text.chars().count() } else { 4 };
            std::iter::repeat(ch).take(len.max(1)).collect()
        }
        Strategy::Mask => mask(text, rule),
        Strategy::Delete => String::new(),
    }
}

fn mask(text: &str, rule: &CategoryRule) -> String {
    let ch = rule.mask_char.chars().next().unwrap_or('X');
    let total = text.chars().filter(|c| c.is_alphanumeric()).count();
    let keep_from = total.saturating_sub(rule.keep_last);
    let mut seen = 0;
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() {
            if seen >= keep_from {
                out.push(c);
            } else {
                out.push(ch);
            }
            seen += 1;
        } else {
            out.push(c);
        }
    }
    out
}

fn pseudonym(text: &str, category: Category, parts: &[NamePart], seps: &[String], p: &mut Pseudonymizer, d: &Dictionaries) -> String {
    match category {
        Category::Person => {
            if parts.is_empty() {
                p.last_name(text)
            } else {
                p.person(parts, seps)
            }
        }
        Category::Address => p.address(text),
        Category::PostalCode => p.postal_code(text),
        Category::Date => dates::shift(text, p.date_offset_days).or_else(|| dates::shift_serial(text, p.date_offset_days)).unwrap_or_else(|| p.customer_id(text)),
        Category::Email => p.email(text, d),
        Category::Phone => p.phone(text),
        Category::Iban => p.iban(text),
        Category::CreditCard => p.credit_card(text),
        Category::TaxId => p.tax_id(text),
        Category::Insurance => p.insurance(text),
        Category::Plate => p.plate(text),
        Category::Ip => p.ip(text),
        Category::CustomerId => {
            if text.chars().any(|c| c.is_ascii_digit()) {
                p.customer_id(text)
            } else {
                p.custom_word(text)
            }
        }
    }
}

/// Ersetzungen (Byte-Bereiche, sortiert, überlappungsfrei) in den Text einsetzen.
pub fn splice(text: &str, edits: &[(usize, usize, String)]) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut pos = 0;
    for (s, e, r) in edits {
        if *s < pos {
            continue;
        }
        out.push_str(&text[pos..*s]);
        out.push_str(r);
        pos = *e;
    }
    out.push_str(&text[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::World;

    #[test]
    fn strategies() {
        let mut p = Pseudonymizer::new(World::builtin("de"), "s", 0, None);
        let d = Dictionaries::builtin();
        let mut rule = CategoryRule::default();
        rule.strategy = Strategy::Placeholder;
        assert_eq!(replacement("Anna", Category::Person, &[], &[], &rule, &mut p, &d), "[PERSON 1]");
        assert_eq!(replacement("Jonas", Category::Person, &[], &[], &rule, &mut p, &d), "[PERSON 2]");
        assert_eq!(replacement("anna", Category::Person, &[], &[], &rule, &mut p, &d), "[PERSON 1]");
        rule.strategy = Strategy::Redact;
        assert_eq!(replacement("Anna", Category::Person, &[], &[], &rule, &mut p, &d), "████");
        rule.strategy = Strategy::Mask;
        rule.keep_last = 4;
        assert_eq!(replacement("DE89 3704 0044 0532 0130 00", Category::Iban, &[], &[], &rule, &mut p, &d), "XXXX XXXX XXXX XXXX XX30 00");
        rule.strategy = Strategy::Delete;
        assert_eq!(replacement("Anna", Category::Person, &[], &[], &rule, &mut p, &d), "");
    }

    #[test]
    fn splicing() {
        let t = "Hallo Anna, hallo Jonas.";
        let out = splice(t, &[(6, 10, "Lena".into()), (18, 23, "Noah".into())]);
        assert_eq!(out, "Hallo Lena, hallo Noah.");
    }
}
