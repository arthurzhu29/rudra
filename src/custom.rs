use crate::document3::*;
use crate::ui::*;

// custom-larksson
// File: tree<statement>
// 0: Statement: set: var, value | print: value
// 1: var: tree<value>
// 2: value: number (symbol) | inline-var | tree<pair>
// 3: pair: value, value
// 4: [helper]: '.' value

#[allow(unused)]
macro_rules! structo {
    ($name:expr $(, ($name2:expr $(, $field:expr)* $(,)?))* $(,)?) => {
        StructDef {
            name: $name.to_string(),
            variants: vec![$(
                VariantDef {
                    name: $name2.to_string(),
                    fields: vec![$($field.into(), )*],
                },
            )*],
        }
    };
}

#[allow(unused)]
macro_rules! types {
    (
        $(
            $name:expr => {
                $(
                    $(;)?
                    $(
                        $variant:expr => (
                            $($($field:expr),+ $(,)?)?
                        )
                    );+
                )?
            }
        );* $(;)?
    ) => {
        vec![
            $(
                StructDef {
                    name: $name.to_string(),
                    variants: vec![
                        $($(
                            VariantDef {
                                name: $variant.to_string(),
                                fields: vec![
                                    $($($field.into(),)+)?
                                ],
                            },
                        )+)?
                    ],
                },
            )*
        ]
    };
}

impl From<&str> for FieldDef {
    fn from(item: &str) -> FieldDef {
        FieldDef {
            name: item.to_string(),
            value: CellValue::Symbol,
            is_tree: false,
        }
    }
}
impl From<(&str, usize)> for FieldDef {
    fn from(item: (&str, usize)) -> FieldDef {
        FieldDef {
            name: item.0.to_string(),
            value: CellValue::Struct(item.1),
            is_tree: false,
        }
    }
}
impl From<[&str; 1]> for FieldDef {
    fn from(item: [&str; 1]) -> FieldDef {
        FieldDef {
            name: item[0].to_string(),
            value: CellValue::Symbol,
            is_tree: true,
        }
    }
}
impl From<[(&str, usize); 1]> for FieldDef {
    fn from(item: [(&str, usize); 1]) -> FieldDef {
        FieldDef {
            name: item[0].0.to_string(),
            value: CellValue::Struct(item[0].1),
            is_tree: true,
        }
    }
}

pub fn sample_types() -> Types {
    // no variants -> panic upon construction
    let _dummy = StructDef {
        name: "placeholder; todo".to_string(),
        variants: vec![VariantDef { name: "dummy".to_string(), fields: vec![] }]
    };
    Types {
        types: types! {
            "statement" => {
                ; "set_statement" => (("var", 1), ("value", 2))
                ; "print_statement" => ( ("value", 2) )
            };
            "var" => { "tree<value>" => ([("('.' value)* '.'", 4)]) };
            "value" => {
                ; "number" => ("number")
                ; "inline_var" => (("var", 1))
                ; "list" => ([("tree<pair>", 3)])
            };
            "pair" => { "value ':' value" => (("value", 2), ("value", 2)) };
            "'.' value" => { "'.' value" => (("'.' value", 2)) };
        },
        rim: [("file", 0)].into(),
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