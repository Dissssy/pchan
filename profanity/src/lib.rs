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
            let record: ProfanityWord = result?;
            list.push(record);
        }
        Ok(Profanity { list })
    }
    pub fn check_profanity(&self, text: &str) -> Option<ProfanityWord> {
        let text = text.to_lowercase();
        for word in self.list.iter() {
            if text.contains(&word.word) {
                return Some(word.clone());
            }
        }
        None
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

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum SeverityDescription {
    Mild,
    Severe,
    Strong,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
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
