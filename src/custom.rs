use crate::document3::*;
use crate::ui::*;

// custom-larksson
// File: tree<statement>
// 0: Statement: set: var, value | print: value
// 1: var: tree<value>
// 2: value: number (symbol) | inline-var | tree<pair>
// 3: pair: value, value
// 4: [helper]: '.' value

pub fn sample_types() -> Types {
    // no variants -> panic upon construction
    let _dummy = StructDef {
        name: "placeholder; todo".to_string(),
        variants: vec![
            VariantDef {
                name: "dummy".to_string(),
                fields: vec![],
            }
        ],
    };
    Types {
        types: vec![
            StructDef {
                name: "statement".to_string(),
                variants : vec![
                    VariantDef {
                        name: "set_statement".to_string(),
                        fields: vec![
                            FieldDef { name: "var".to_string(), value: CellValue::Struct(1), is_tree: false },
                            FieldDef { name: "value".to_string(), value: CellValue::Struct(2), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "print_statement".to_string(),
                        fields: vec![
                            FieldDef { name: "value".to_string(), value: CellValue::Struct(2), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "var".to_string(),
                variants : vec![
                    VariantDef {
                        name: "tree<value>".to_string(),
                        fields: vec![
                            FieldDef { name: "('.' value)* '.'".to_string(), value: CellValue::Struct(4), is_tree: true },
                        ],
                    },
                ],
            },
            StructDef {
                name: "value".to_string(),
                variants : vec![
                    VariantDef {
                        name: "number".to_string(),
                        fields: vec![
                            FieldDef { name: "number".to_string(), value: CellValue::Symbol, is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "inline_var".to_string(),
                        fields: vec![
                            FieldDef { name: "var".to_string(), value: CellValue::Struct(1), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "list".to_string(),
                        fields: vec![
                            FieldDef { name: "tree<pair>".to_string(), value: CellValue::Struct(3), is_tree: true },
                        ],
                    },
                ],
            },
            StructDef {
                name: "pair".to_string(),
                variants : vec![
                    VariantDef {
                        name: "value ':' value".to_string(),
                        fields: vec![
                            FieldDef { name: "value".to_string(), value: CellValue::Struct(2), is_tree: false },
                            FieldDef { name: "value".to_string(), value: CellValue::Struct(2), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "'.' value".to_string(),
                variants : vec![
                    VariantDef {
                        name: "'.' value".to_string(),
                        fields: vec![
                            FieldDef { name: "'.' value".to_string(), value: CellValue::Struct(2), is_tree: false },
                        ],
                    },
                ],
            },
        ],
        rim: FieldDef {
            name: "file".to_string(),
            value: CellValue::Struct(0),
            is_tree: true,
        },
    }
}

pub const STATIC_BUILDER: StaticBuilder = StaticBuilder {
    root: "\n",
    data: &[
        // statement
        &[
            // set_statement
            // var '=' value
            (&[("", None), (" = ", None)], ""),
            // print_statement
            // 'print' value
            (&[("print ", None)], ""),
        ],
        // var
        &[
            // ('.' value)* '.'
            (&[("", Some(""))], "."),
        ],
        // value
        &[
            // number
            (&[("", None)], ""),
            // inline_var
            (&[("(", None)], ")"),
            // list
            (&[("{ ", Some(", "))], " }"),
        ],
        // pair
        &[
            // value ':' value
            (&[("", None), (": ", None)], ""),
        ],
        // '.' value
        &[
            (&[(".", None)], ""),
        ],
    ],
};