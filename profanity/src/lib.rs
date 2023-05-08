use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Profanity {
    pub list: Vec<ProfanityWord>,
}

impl Profanity {
    pub fn load_csv(path: &str) -> Result<Self> {
        let mut rdr = csv::Reader::from_path(path)?;
        let mut list = Vec::new();
        for result in rdr.deserialize() {
            let mut record: ProfanityWord = result?;
            record.word = record.word.to_lowercase();
            list.push(record);
        }
        Ok(Profanity { list })
    }
    pub fn check_profanity(&self, text: &str) -> Vec<ProfanityWord> {
        let text = text.to_lowercase();
        let mut p = self
            .list
            .iter()
            .flat_map(|x| {
                if text.contains(&x.word) {
                    Some(x.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<ProfanityWord>>();
        p.sort_by(|a, b| b.word.len().cmp(&a.word.len()));
        p
    }
    pub fn get_all(&self, category: Category) -> Vec<ProfanityWord> {
        self.list
            .iter()
            .filter(|x| x.category == category)
            .cloned()
            .collect()
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ProfanityWord {
    #[serde(rename = "text")]
    pub word: String,
    #[serde(rename = "canonical_form_1")]
    pub canonical_form: String,
    #[serde(rename = "canonical_form_2")]
    pub canonical_form_2: Option<String>,
    #[serde(rename = "canonical_form_3")]
    pub canonical_form_3: Option<String>,
    #[serde(rename = "category_1")]
    pub category: Category,
    #[serde(rename = "category_2")]
    pub category_2: Option<Category>,
    #[serde(rename = "category_3")]
    pub category_3: Option<Category>,
    #[serde(rename = "severity_rating")]
    pub severity: f32,
    #[serde(rename = "severity_description")]
    pub severity_description: SeverityDescription,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SeverityDescription {
    Mild,
    Severe,
    Strong,
}

impl PartialOrd for SeverityDescription {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SeverityDescription {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Mild, Self::Mild) => std::cmp::Ordering::Equal,
            (Self::Mild, Self::Severe) => std::cmp::Ordering::Less,
            (Self::Mild, Self::Strong) => std::cmp::Ordering::Less,
            (Self::Severe, Self::Mild) => std::cmp::Ordering::Greater,
            (Self::Severe, Self::Severe) => std::cmp::Ordering::Equal,
            (Self::Severe, Self::Strong) => std::cmp::Ordering::Less,
            (Self::Strong, Self::Mild) => std::cmp::Ordering::Greater,
            (Self::Strong, Self::Severe) => std::cmp::Ordering::Greater,
            (Self::Strong, Self::Strong) => std::cmp::Ordering::Equal,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Category {
    #[serde(rename = "sexual anatomy / sexual acts")]
    SexualAnatomy,
    #[serde(rename = "sexual orientation / gender")]
    SexualIdentity,
    #[serde(rename = "bodily fluids / excrement")]
    BodilyFluids,
    #[serde(rename = "racial / ethnic slurs")]
    RacialSlurs,
    #[serde(rename = "animal references")]
    AnimalReferences,
    #[serde(rename = "other / general insult")]
    Other,
    #[serde(rename = "mental disability")]
    MentalDisability,
    #[serde(rename = "political")]
    Political,
    #[serde(rename = "physical attributes")]
    PhysicalAttributes,
    #[serde(rename = "religious offense")]
    ReligiousOffense,
    #[serde(rename = "physical disability")]
    PhysicalDisability,
}

pub fn replace_possible_profanity<F>(string: String, profanity: &Profanity, f: F) -> String
where
    F: Fn() -> String,
{
    let scrunkly = profanity.check_profanity(&string);
    let mut banned_categories = HashMap::new();
    banned_categories.insert(Category::RacialSlurs, SeverityDescription::Mild);
    banned_categories.insert(Category::SexualIdentity, SeverityDescription::Severe);

    let mut orig_chars = string.chars().collect::<Vec<char>>();

    for word in scrunkly {
        let categories = vec![
            Some(word.category), /* word.category_2, word.category_3 */
        ];
        if categories.iter().any(|x| {
            if let Some(x) = &x {
                banned_categories
                    .get(x)
                    .filter(|y| **y >= word.severity_description)
                    .is_some()
            } else {
                false
            }
        }) {
            let sequence = &word.word.chars().collect::<Vec<char>>();
            // find every index of the sequence by iterating over windows of the chars vec
            let lower_chars = orig_chars
                .iter()
                .collect::<String>()
                .to_lowercase()
                .chars()
                .collect::<Vec<char>>();

            let mut indices = Vec::new();
            for (index, _) in lower_chars
                .windows(sequence.len())
                .enumerate()
                .filter(|(_, stringy)| stringy == sequence)
            {
                indices.push(index);
            }
            // replace the range of indices with the replacement strings chars. WE NEED TO DO THIS FROM LARGEST TO SMALLEST OTHERWISE THE INDICES WILL BE WRONG
            indices.sort_by(|a, b| b.cmp(a));
            for index in indices {
                orig_chars.splice(index..(index + sequence.len()), f().chars());
            }
        }
    }
    orig_chars.into_iter().collect::<String>()
}
