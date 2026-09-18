//! Prüfziffern: IBAN (ISO 7064 Mod 97-10), Luhn (Kreditkarten), deutsche
//! Steuer-ID (ISO 7064 Mod 11,10) und Sozialversicherungsnummer.

/// IBAN ohne Leerzeichen prüfen.
pub fn iban_valid(compact: &str) -> bool {
    let s: String = compact.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if s.len() < 15 || s.len() > 34 {
        return false;
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    iban_mod97(&s) == 1
}

fn iban_mod97(s: &str) -> u32 {
    // Land + Prüfziffer ans Ende, Buchstaben → Zahlen (A=10 … Z=35), dann Mod 97.
    let rotated = format!("{}{}", &s[4..], &s[..4]);
    let mut rem: u32 = 0;
    for c in rotated.chars() {
        let v = if c.is_ascii_digit() { c as u32 - '0' as u32 } else { c.to_ascii_uppercase() as u32 - 'A' as u32 + 10 };
        if v >= 10 {
            rem = (rem * 100 + v) % 97;
        } else {
            rem = (rem * 10 + v) % 97;
        }
    }
    rem
}

/// Prüfziffern für Land + BBAN berechnen (`DE` + BBAN → `DE89…`).
pub fn iban_with_check(country: &str, bban: &str) -> String {
    let probe = format!("{}00{}", country.to_uppercase(), bban.to_uppercase());
    let rem = iban_mod97(&probe);
    let check = 98 - rem;
    format!("{}{:02}{}", country.to_uppercase(), check, bban.to_uppercase())
}

/// Erwartete IBAN-Länge je Land (für Treffer ohne gültige Prüfziffer).
pub fn iban_length(country: &str) -> Option<usize> {
    Some(match country {
        "AT" => 20,
        "BE" => 16,
        "CH" => 21,
        "CZ" => 24,
        "DE" => 22,
        "DK" => 18,
        "ES" => 24,
        "FI" => 18,
        "FR" => 27,
        "GB" => 22,
        "HU" => 28,
        "IE" => 22,
        "IT" => 27,
        "LI" => 21,
        "LU" => 20,
        "NL" => 18,
        "NO" => 15,
        "PL" => 28,
        "PT" => 25,
        "SE" => 24,
        "SK" => 24,
        _ => return None,
    })
}

/// Luhn-Prüfung über alle Ziffern eines Strings (Trennzeichen werden ignoriert).
pub fn luhn_valid(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 12 {
        return false;
    }
    luhn_sum(&digits) % 10 == 0
}

fn luhn_sum(digits: &[u32]) -> u32 {
    let mut sum = 0;
    let mut double = false;
    for &d in digits.iter().rev() {
        let mut v = d;
        if double {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
        double = !double;
    }
    sum
}

/// Luhn-Prüfziffer für eine Ziffernfolge (ohne Prüfziffer) berechnen.
pub fn luhn_check_digit(digits: &[u32]) -> u32 {
    let mut with = digits.to_vec();
    with.push(0);
    (10 - luhn_sum(&with) % 10) % 10
}

/// Deutsche Steuer-Identifikationsnummer (11 Ziffern).
pub fn tax_id_valid(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 11 || digits[0] == 0 {
        return false;
    }
    // In den ersten zehn Ziffern kommt genau eine Ziffer doppelt (oder dreifach) vor.
    let mut counts = [0u8; 10];
    for &d in &digits[..10] {
        counts[d as usize] += 1;
    }
    let multi = counts.iter().filter(|&&c| c >= 2).count();
    let zeros = counts.iter().filter(|&&c| c == 0).count();
    if multi != 1 || (zeros != 1 && zeros != 2) {
        return false;
    }
    tax_id_check_digit(&digits[..10]) == digits[10]
}

/// ISO 7064 Mod 11,10 über die ersten zehn Ziffern.
pub fn tax_id_check_digit(first_ten: &[u32]) -> u32 {
    let mut product = 10u32;
    for &d in first_ten {
        let mut sum = (d + product) % 10;
        if sum == 0 {
            sum = 10;
        }
        product = (sum * 2) % 11;
    }
    let check = 11 - product;
    if check == 10 {
        0
    } else {
        check
    }
}

/// Deutsche Sozialversicherungsnummer (`12 190367 K 003`).
pub fn svnr_valid(s: &str) -> bool {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if compact.len() != 12 {
        return false;
    }
    let b = compact.as_bytes();
    if !b[..8].iter().all(|c| c.is_ascii_digit()) || !b[8].is_ascii_alphabetic() || !b[9..].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let letter = (b[8] - b'A' + 1) as u32;
    let mut digits: Vec<u32> = b[..8].iter().map(|c| (c - b'0') as u32).collect();
    digits.push(letter / 10);
    digits.push(letter % 10);
    digits.push((b[9] - b'0') as u32);
    digits.push((b[10] - b'0') as u32);
    let weights = [2, 1, 2, 5, 7, 1, 2, 1, 2, 1, 2, 1];
    let sum: u32 = digits.iter().zip(weights.iter()).map(|(d, w)| digit_sum(d * w)).sum();
    sum % 10 == (b[11] - b'0') as u32
}

fn digit_sum(mut n: u32) -> u32 {
    let mut s = 0;
    while n > 0 {
        s += n % 10;
        n /= 10;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iban() {
        assert!(iban_valid("DE89370400440532013000"));
        assert!(iban_valid("DE89 3704 0044 0532 0130 00"));
        assert!(!iban_valid("DE88370400440532013000"));
        assert_eq!(iban_with_check("DE", "370400440532013000"), "DE89370400440532013000");
        assert!(iban_valid("GB82WEST12345698765432"));
    }

    #[test]
    fn luhn() {
        assert!(luhn_valid("4539 1488 0343 6467"));
        assert!(!luhn_valid("4539 1488 0343 6468"));
        let d: Vec<u32> = "453914880343646".chars().map(|c| c.to_digit(10).unwrap()).collect();
        assert_eq!(luhn_check_digit(&d), 7);
    }

    #[test]
    fn tax_id() {
        assert!(tax_id_valid("86095742719"));
        assert!(!tax_id_valid("86095742718"));
        assert!(!tax_id_valid("12345678901"));
    }

    #[test]
    fn svnr() {
        assert!(svnr_valid("65 170839 J 003"));
        assert!(!svnr_valid("65 170839 J 008"));
    }
}
