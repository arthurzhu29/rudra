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
fn struct_one(name: &str, variant: VariantDef) -> StructDef {
    StructDef {
        name: name.to_string(),
        variants: vec![variant],
    }
}
#[allow(unused)]
fn struct_two(name: &str, variant_one: VariantDef, variant_two: VariantDef) -> StructDef {
    StructDef {
        name: name.to_string(),
        variants: vec![variant_one, variant_two],
    }
}
#[allow(unused)]
fn struct_three(name: &str, variant_one: VariantDef, variant_two: VariantDef, variant_three: VariantDef) -> StructDef {
    StructDef {
        name: name.to_string(),
        variants: vec![variant_one, variant_two, variant_three],
    }
}
#[allow(unused)]
fn struct_many(name: &str, variants: Vec<VariantDef>) -> StructDef {
    StructDef {
        name: name.to_string(),
        variants,
    }
}
#[allow(unused)]
fn variant_zero(name: &str) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        fields: vec![],
    }
}
#[allow(unused)]
fn variant_one(name: &str, field: FieldDef) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        fields: vec![field],
    }
}
#[allow(unused)]
fn variant_two(name: &str, field_one: FieldDef, field_two: FieldDef) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        fields: vec![field_one, field_two],
    }
}
#[allow(unused)]
fn variant_three(name: &str, field_one: FieldDef, field_two: FieldDef, field_three: FieldDef) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        fields: vec![field_one, field_two, field_three],
    }
}
#[allow(unused)]
fn variant_many(name: &str, fields: Vec<FieldDef>) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        fields,
    }
}
#[allow(unused)]
fn field_struct(name: &str, struct_id: usize) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        value: CellValue::Struct(struct_id),
        is_tree: false,
    }
}
#[allow(unused)]
fn field_structs(name: &str, struct_id: usize) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        value: CellValue::Struct(struct_id),
        is_tree: true,
    }
}
#[allow(unused)]
fn field_symbol(name: &str) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        value: CellValue::Symbol,
        is_tree: false,
    }
}
#[allow(unused)]
fn field_symbols(name: &str) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        value: CellValue::Symbol,
        is_tree: true,
    }
}
#[allow(unused)]
fn variant_single_struct(name: &str, field_name: &str, struct_id: usize) -> VariantDef {
    variant_one(name, field_struct(field_name, struct_id))
}
#[allow(unused)]
fn variant_single_symbol(name: &str, field_name: &str) -> VariantDef {
    variant_one(name, field_symbol(field_name))
}
#[allow(unused)]
fn struct_wrapper(name: &str, variant_name: &str, field_name: &str, struct_id: usize) -> StructDef {
    struct_one(name, variant_single_struct(variant_name, field_name, struct_id))
}

pub fn sample_types() -> Types {
    // no variants -> panic upon construction
    let _dummy = struct_one("placeholder; todo", variant_zero("dummy"));
    Types {
        types: vec![
            struct_two(
                "statement",
                variant_two("set_statement", field_struct("var", 1), field_struct("value", 2)),
                variant_single_struct("print_statement", "value", 2),
            ),
            struct_one("var", variant_one("tree<value>", field_structs("('.' value)* '.'", 4))),
            struct_three(
                "value",
                variant_single_symbol("number", "number"),
                variant_single_struct("inline_var", "var", 1),
                variant_one("list", field_structs("tree<pair>", 3)),
            ),
            struct_one("pair", variant_two("value ':' value", field_struct("value", 2), field_struct("value", 2))),
            struct_wrapper("'.' value", "'.' value", "'.' value", 2),
        ],
        rim: field_structs("file", 0),
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