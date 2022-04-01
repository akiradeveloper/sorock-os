use crate::*;
use std::path::PathBuf;
enum Store {
    Memory,
    Directory { path: PathBuf },
}
