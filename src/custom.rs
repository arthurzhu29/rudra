
use crate::document3::*;
use crate::ui::*;

/// A small sample schema so the MVP shows something interesting: one struct
/// with a two-symbol variant (exercises variant cycling and field rendering)
/// and one struct with a tree-typed field (exercises nested grids).
pub fn sample_types() -> Types {
    Types {
        types: vec![
            // struct 0: "Pair"
            StructDef {
                name: "Pair".to_string(),
                variants: vec![
                    // variant 1: two symbol fields
                    VariantDef {
                        name: "xy".to_string(),
                        fields: vec![
                            FieldDef { name: "x".to_string(), value: CellValue::Symbol, is_tree: false },
                            FieldDef { name: "y".to_string(), value: CellValue::Symbol, is_tree: false },
                        ],
                    },
                ],
            },
            // struct 1: "Box"
            StructDef {
                name: "Box".to_string(),
                variants: vec![
                    VariantDef {
                        name: "grid".to_string(),
                        fields: vec![FieldDef {
                            name: "items".to_string(),
                            value: CellValue::Symbol,
                            is_tree: true,
                        }],
                    },
                ],
            },
        ],
        rim: FieldDef {
            name: "canvas".to_string(),
            value: CellValue::Struct(0),
            is_tree: false,
        },
    }
}
pub const STATIC_BUILDER: StaticBuilder = StaticBuilder {
    root: ", ",
    data: &[
        &[
            (
                &[
                    ("Pair { x: \"", None),
                    ("\", y: \"", None),
                ],
                "\" }",
            ),
        ],
        &[
            (
                &[
                    ("Box(\"", Some("")),
                ],
                "\")",
            ),
        ],
    ],
};