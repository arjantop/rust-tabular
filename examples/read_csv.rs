extern crate tabular;

use tabular::dsv;

fn main() {
    let path = Path::new("data/short.csv");
    for row in dsv::from_file(dsv::CSV, &path) {
        println!("row = {}", row);
    }
}
