use std::io;

use crate::model::CSVRecord;

pub fn csv_stream<R: io::Read>(buffer: R) -> impl Iterator<Item = Result<CSVRecord, csv::Error>> {
    let reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All) // Strip Whitespace
        .from_reader(buffer);

    reader.into_deserialize::<CSVRecord>()
}

#[cfg(test)]
mod tests {
    use crate::model::TxType;

    #[test]
    fn trimming_test() {
        let test_data = r#" type,  client,  tx,  amount
deposit,  1,  1,  100.0
"#;

        let mut reader = super::csv_stream(test_data.as_bytes());

        let record = reader.next().unwrap().unwrap();
        assert_eq!(record.r#type, TxType::Deposit);
        assert_eq!(record.client, 1);
        assert_eq!(record.tx, 1);
        assert!(record.amount.is_some());
    }
}
