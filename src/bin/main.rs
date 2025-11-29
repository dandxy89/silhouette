use std::io;
use std::io::Write;

use silhouette::{file_reader::csv_stream, ledger::engine::PaymentsEngine};

fn main() {
    let file_path = std::env::args().nth(1).expect("No file_path was provided");

    let mut payment_engine = PaymentsEngine::default();
    let mut stderr = io::stderr().lock();

    for csv_record in csv_stream(&file_path) {
        if let Ok(record) = csv_record {
            if let Err(err) = payment_engine.process_csv_record(record) {
                let _ = writeln!(stderr, "Error processing Transaction due to {err:?}");
            }
        } else {
            let _ = writeln!(stderr, "Error reading csv record: {csv_record:?}");
        }
    }
}
