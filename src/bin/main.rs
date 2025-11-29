use std::io::{BufReader, Write};
use std::{fs::File, io};

use silhouette::{file_reader::csv_stream, ledger::engine::PaymentsEngine};

fn main() {
    let file_path = std::env::args().nth(1).expect("No file_path was provided");
    let Ok(file) = File::open(&file_path) else {
        panic!("Failed to open file at path: {file_path}");
    };
    let buffer = BufReader::new(file);

    let mut payment_engine = PaymentsEngine::default();
    let mut stderr = io::stderr().lock();

    for csv_record in csv_stream(buffer) {
        if let Ok(record) = csv_record {
            if let Err(err) = payment_engine.process_csv_record(record) {
                let _ = writeln!(stderr, "Error processing Transaction due to {err:?}");
            }
        } else {
            let _ = writeln!(stderr, "Error reading csv record: {csv_record:?}");
        }
    }
}
