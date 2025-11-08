pub mod format;
pub mod multi;
pub mod single;

mod shared;

pub use single::Match;

pub use format::{FormatOption, format_folder_matches};

pub use single::{
    find_matches_in_file as find_single_in_file, find_matches_in_folder as find_single_in_folder,
};
