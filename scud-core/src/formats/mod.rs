//! Task graph serialization formats
//!
//! This module provides parsers and serializers for different
//! task storage formats.

mod scg;

pub use scg::{natural_sort_ids, parse_scg, serialize_scg, validate_weave};

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    /// Legacy JSON format
    Json,
    /// SCUD Graph format (.scg)
    Scg,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Format::Json),
            "scg" => Some(Format::Scg),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Scg => "scg",
        }
    }
}
