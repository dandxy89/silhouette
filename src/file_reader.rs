use std::fs::File;

use crate::model::CSVRecord;

/// TODO: Whitespace and decimal precision up to 4dps
pub fn csv_stream(path: &str) -> impl Iterator<Item = Result<CSVRecord, csv::Error>> {
    let file = File::open(path).unwrap();
    let reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);
    reader.into_deserialize::<CSVRecord>()
}
