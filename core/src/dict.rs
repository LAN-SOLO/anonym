//! Wörterbücher: Erkennungslisten (eingebettet) und Namenswelten für Pseudonyme.

use crate::model::Gender;
use std::collections::{HashMap, HashSet};

const FIRST_NAMES: &str = include_str!("../data/first_names.txt");
const LAST_NAMES: &str = include_str!("../data/last_names.txt");
const CITIES: &str = include_str!("../data/cities.txt");
const STREETS: &str = include_str!("../data/streets.txt");

const WORLD_FILES: [(&str, &str); 9] = [
    ("de", include_str!("../data/worlds/de.txt")),
    ("en", include_str!("../data/worlds/en.txt")),
    ("fr", include_str!("../data/worlds/fr.txt")),
    ("es", include_str!("../data/worlds/es.txt")),
    ("tr", include_str!("../data/worlds/tr.txt")),
    ("ja", include_str!("../data/worlds/ja.txt")),
    ("fantasy", include_str!("../data/worlds/fantasy.txt")),
    ("scifi", include_str!("../data/worlds/scifi.txt")),
    ("medieval", include_str!("../data/worlds/medieval.txt")),
];

pub const WORLD_IDS: [&str; 9] = ["de", "en", "fr", "es", "tr", "ja", "fantasy", "scifi", "medieval"];

#[derive(Debug, Clone, Copy)]
pub struct FirstName {
    pub gender: Gender,
    /// Nur mit Nachname oder Anrede als Person werten (Mark, Bill, Frank, …).
    pub ambiguous: bool,
}

/// Erkennungslisten — Schlüssel immer kleingeschrieben.
pub struct Dictionaries {
    pub first_names: HashMap<String, FirstName>,
    pub last_names: HashSet<String>,
    pub cities: HashSet<String>,
    pub streets: Vec<String>,
}

fn lines(src: &str) -> impl Iterator<Item = &str> {
    src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
}

impl Dictionaries {
    pub fn builtin() -> Dictionaries {
        let mut first_names = HashMap::new();
        for line in lines(FIRST_NAMES) {
            let mut parts = line.split(';');
            let name = parts.next().unwrap_or("").trim();
            let gender = match parts.next().unwrap_or("u").trim() {
                "m" => Gender::M,
                "f" => Gender::F,
                _ => Gender::U,
            };
            let ambiguous = parts.next().map(|p| p.trim() == "?").unwrap_or(false);
            if !name.is_empty() {
                first_names.insert(name.to_lowercase(), FirstName { gender, ambiguous });
            }
        }
        let last_names = lines(LAST_NAMES).map(|l| l.to_lowercase()).collect();
        let cities = lines(CITIES).map(|l| l.to_lowercase()).collect();
        let streets = lines(STREETS).map(|l| l.to_string()).collect();
        Dictionaries { first_names, last_names, cities, streets }
    }

    pub fn first_name(&self, word: &str) -> Option<FirstName> {
        self.first_names.get(&word.to_lowercase()).copied()
    }

    pub fn is_last_name(&self, word: &str) -> bool {
        self.last_names.contains(&word.to_lowercase())
    }

    pub fn is_city(&self, word: &str) -> bool {
        self.cities.contains(&word.to_lowercase())
    }
}

/// Eine Namenswelt: Ersatznamen nach Genus, Nachnamen, Straßen.
#[derive(Debug, Clone)]
pub struct World {
    pub id: String,
    pub female: Vec<String>,
    pub male: Vec<String>,
    pub last: Vec<String>,
    pub streets: Vec<String>,
}

impl World {
    pub fn parse(id: &str, src: &str) -> World {
        let mut w = World { id: id.to_string(), female: vec![], male: vec![], last: vec![], streets: vec![] };
        let mut section = "";
        for line in lines(src) {
            if line.starts_with('[') && line.ends_with(']') {
                section = match &line[1..line.len() - 1] {
                    "f" => "f",
                    "m" => "m",
                    "last" => "last",
                    "street" => "street",
                    _ => "",
                };
                continue;
            }
            match section {
                "f" => w.female.push(line.to_string()),
                "m" => w.male.push(line.to_string()),
                "last" => w.last.push(line.to_string()),
                "street" => w.streets.push(line.to_string()),
                _ => {}
            }
        }
        if w.streets.is_empty() {
            w.streets = lines(STREETS).map(|l| l.to_string()).collect();
        }
        w
    }

    /// Eingebaute Welt laden; unbekannte Kennung → `de`.
    pub fn builtin(id: &str) -> World {
        let (wid, src) = WORLD_FILES.iter().copied().find(|(k, _)| *k == id).unwrap_or(WORLD_FILES[0]);
        World::parse(wid, src)
    }

    pub fn first_names(&self, gender: Gender) -> &[String] {
        match gender {
            Gender::F => &self.female,
            Gender::M => &self.male,
            Gender::U => {
                if self.female.len() >= self.male.len() {
                    &self.female
                } else {
                    &self.male
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads() {
        let d = Dictionaries::builtin();
        assert_eq!(d.first_name("Anna").unwrap().gender, Gender::F);
        assert!(d.first_name("Frank").unwrap().ambiguous);
        assert!(d.is_last_name("Berger"));
        assert!(d.is_city("Köln"));
        for id in WORLD_IDS {
            let w = World::builtin(id);
            assert!(w.female.len() >= 30 && w.male.len() >= 30 && w.last.len() >= 30 && !w.streets.is_empty(), "world {id}");
        }
    }
}
