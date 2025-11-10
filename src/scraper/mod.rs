use include_dir::{Dir, include_dir};
use strum::{Display, EnumString, VariantArray};

pub static SCRAPE_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/scraping");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, VariantArray)]
pub enum ScrapingTarget {
    #[strum(serialize = "Surviving as a 'Talent' in the Primal Saint Demonic Sect")]
    TalentInDemonicSect,
}

pub mod scraper2322424255;
