//! Rudra — entry point.
//!
//! The Bevy view layer (impl-spec-v2 §6 onward) is not yet built. For now this
//! exercises the document core: it builds a sample document and runs the
//! load-time integrity check.
//! 

use rudra::document;

use document::*;

fn main() {
    // A tiny schema: one struct "Note" with the nameless empty variant 0 and a
    // variant 1 carrying a single Symbol field.
    let schema = Schema {
        structs: vec![StructDef {
            name: "Note".into(),
            variants: vec![
                VariantDef { name: String::new(), fields: vec![] },
                VariantDef {
                    name: "text".into(),
                    fields: vec![FieldDef {
                        name: "body".into(),
                        elem: TypeRef::Symbol,
                        is_tree: false,
                    }],
                },
            ],
        }],
    };

    // A document whose Rim canvas is a Tree<Symbol>.
    let root_field = FieldDef { name: "canvas".into(), elem: TypeRef::Symbol, is_tree: true };
    let doc = Document::new(schema, root_field);

    match check_integrity(&doc) {
        Ok(()) => println!(
            "rudra: document core OK — schema has {} struct(s), Rim is a {}.",
            doc.schema.structs.len(),
            match &doc.rim {
                CellContent::Tree(_) => "tree",
                CellContent::Value(_) => "value",
            },
        ),
        Err(IntegrityError(msg)) => {
            eprintln!("rudra: integrity check failed: {msg}");
            std::process::exit(1);
        }
    }
}