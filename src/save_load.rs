use bincode;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use crate::document3::*;

pub fn load(types: &Types) -> Document {
    let Ok(file) = File::open("save") else {
        return Document::new(types);
    };
    let mut reader = BufReader::new(file);
    bincode::serde::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap()

}

fn save(document: &Document) {
    let file = File::create("save").unwrap();
    let mut writer = BufWriter::new(file);
    bincode::serde::encode_into_std_write(document, &mut writer,  bincode::config::standard()).unwrap();
}

use bevy::prelude::*;
use crate::ui::Doc;
pub fn save_query(doc: Res<Doc>) {
    save(&doc.0)
}