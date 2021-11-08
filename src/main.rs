use passerine::{common::source::Source, compile, run};

pub fn main() {
    // get the path and load the file
    let path = std::env::args_os().nth(1).expect("Usage: <path>");
    let source =
        Source::path(path.as_ref()).map_err(|_| "Error: File could not be read".to_string());

    // compile and run the file at the specified path
    let bytecode = source.and_then(|s| compile(s).map_err(|e| e.to_string()));
    let result = bytecode.and_then(|b| run(b).map_err(|e| e.to_string()));

    // report any errors
    if let Err(error) = result {
        eprintln!("{}", error);
    }
}
