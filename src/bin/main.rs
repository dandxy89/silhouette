use silhouette::file_reader::csv_stream;

fn main() {
    let file_path = std::env::args().nth(1).expect("No file_path was provided");
    for record in csv_stream(&file_path) {
        println!("Record: {record:?}");
    }
}
