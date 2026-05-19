use std::io::{Result as IoResult, Write};
use crate::document3::*;

// per struct variant -> Vec<String>
// String, Field, String, Field, String
// Strings consumed for tree fields as sep, so:
// String, Field(String), String, Field, String -> 4 Strings
// for example. Fields follow definition order.
//
// path: tree | symbol | (struct/variant -> tree-of-fields -> fields) -> loop
pub struct Printing {
    pub root: String, // in case root is a tree
    pub data: Vec<String>,
    pub variant_offset: Vec<usize>, // per variant; len not included
    pub variant_data_offset: Vec<usize>, // per struct
}

// let file = File::create("..").unwrap();

// fn write_message<W: Write>(mut out: W) -> IoResult<()> {
//     Ok(())
// }

// rules:
// struct name is ommited
// variant name is ommitted
// variants printed according to `Printing`
// Symbols printed as just the string
// nothing printed for empty cells; skipped
// absolutely no other additions.
impl Document {
    pub fn write_txt<W: Write>(&self, w: &mut W, printing: &Printing) -> IoResult<()> {
        self.rim.write_txt(w, printing, &printing.root, &mut false)
    }
}

impl Cell {
    // cell_value
    //
    // sep is empty for non-trees.
    // print_sep is unused for non-trees.
    // print_sep starts as false from the top.
    pub fn write_txt<W: Write>(&self, w: &mut W, printing: &Printing, sep: &str, print_sep: &mut bool) -> IoResult<()> {
        if *print_sep {
            w.write_all(sep.as_bytes())?;
        }
        match self {
            Cell::Symbol(s) => {
                *print_sep = true;
                w.write_all(s.as_bytes())
            },
            Cell::Struct(sv) => {
                *print_sep = true;
                sv.write_txt(w, printing)
            },
            Cell::Tree(tree) => tree.write_txt(w, printing, sep, print_sep), // autonomous tree
            _ => panic!(),
        }
    }
}

// tree -> empty | cell_value | tree
// pass on sep
impl Tree {
    // autonomous tree
    // cells can be empty or cell_value
    pub fn write_txt<W: Write>(&self, w: &mut W, printing: &Printing, sep: &str, print_sep: &mut bool) -> IoResult<()> {
        for s in self.contents.iter().filter(|c| !c.is_empty()) {
            s.write_txt(w, printing, sep, print_sep)?;
        }
        Ok(())
    }
}

// get printing val by getting struct_id and variant_id.
impl StructVal {
    pub fn write_txt<W: Write>(&self, w: &mut W, printing: &Printing) -> IoResult<()> {
        let variant_data_offset = printing.variant_data_offset[self.struct_id];
        let variant_offset = printing.variant_offset[variant_data_offset + self.variant_id];
        let mut i = variant_offset;
        w.write_all(printing.data[i].as_bytes())?;
        i += 1;
        let Some(grid) = &self.grid else {
            return Ok(());
        };
        let Cell::Tree(Tree { contents, .. }) = grid.as_ref() else {
            panic!();
        };
        let field_idxs = {
            let mut sortable = contents.iter().enumerate().filter_map(|(i, cell)| if let Cell::Field(fv) = cell {
                Some((i, fv.field_id))
            } else {
                None
            })
                .collect::<Vec<_>>();
            sortable.sort_unstable_by_key(|(_i, field_id)| *field_id);
            sortable.into_iter().map(|(i, _field_id)| i).collect::<Vec<_>>()
        };
        for field_idx in field_idxs {
            let Cell::Field(fv) = &contents[field_idx] else {
                panic!();
            };
            let sep = match fv.value.as_ref() {
                Cell::Tree(_) => {
                    let r = &printing.data[i];
                    i += 1;
                    r
                },
                _ => "",
            };
            fv.value.write_txt(w, printing, sep, &mut false)?;
            w.write_all(printing.data[i].as_bytes())?;
            i += 1;
        }
        Ok(())
    }
}



pub mod test {
    pub struct PrintingBuilder {
        pub root: String,
        pub data: Vec<Vec<(Vec<(String, Option<String>)>, String)>>,
    }
    use crate::custom::STATIC_BUILDER;
    use crate::ui::StaticBuilder;
    use crate::serialization::*;
    pub fn test_serialize(document: &Document) {
        fn _build(pb: PrintingBuilder) -> Printing {
            let mut variant_offset_i = 0;
            let mut variant_data_offset_i = 0;
            let mut variant_offset = vec![];
            let mut variant_data_offset = vec![];
            let mut data = vec![];
            for struct_data in pb.data {
                variant_data_offset.push(variant_data_offset_i);
                for (variant_data, variant_data_end) in struct_data {
                    variant_data_offset_i += 1;
                    variant_offset.push(variant_offset_i);
                    for (s, os) in variant_data {
                        data.push(s);
                        variant_offset_i += 1;
                        if let Some(s) = os {
                            data.push(s);
                            variant_offset_i += 1;
                        }
                    }
                    data.push(variant_data_end);
                    variant_offset_i += 1;
                }
            }
            Printing {
                root: pb.root,
                data,
                variant_offset,
                variant_data_offset,
            }
        }
        fn build_static(pb: StaticBuilder) -> Printing {
            let mut variant_offset_i = 0;
            let mut variant_data_offset_i = 0;
            let mut variant_offset = vec![];
            let mut variant_data_offset = vec![];
            let mut data = vec![];
            for &struct_data in pb.data {
                variant_data_offset.push(variant_data_offset_i);
                for &(variant_data, variant_data_end) in struct_data {
                    variant_data_offset_i += 1;
                    variant_offset.push(variant_offset_i);
                    for &(s, os) in variant_data {
                        data.push(s.to_string());
                        variant_offset_i += 1;
                        if let Some(s) = os {
                            data.push(s.to_string());
                            variant_offset_i += 1;
                        }
                    }
                    data.push(variant_data_end.to_string());
                    variant_offset_i += 1;
                }
            }
            Printing {
                root: pb.root.to_string(),
                data,
                variant_offset,
                variant_data_offset,
            }
        }

        let mut b = std::fs::File::create("test.txt").unwrap();
        writeln!(&mut b, "{:?}", std::time::SystemTime::now()).unwrap();
        document.write_txt(&mut b, &build_static(STATIC_BUILDER)).unwrap();
    }
}