
extern crate tabular;

use tabular::dsv;

fn main() {
    let path = Path::new("data/short.tsv");
    for row in dsv::from_file(dsv::TSV, &path) {
        println!("row = {}", row);
    }
}
