use crate::document3::*;
use crate::ui::*;

// PYTHON
// python programs are made of: statements, expressions, blocks, indentation
// comments: single-line via `#`, multiline via `"""` (that's actually a multiline string)
// variables and assignment, including multiple assignment and augmented assignment.
// numbers: int, float, complex
// strings: "x" or 'x', or """x""" for multiline
// Booloeans: True, False
// None
// Lists: [1, 2, 3]
// methods: .append(4)
// tuples: (3, 4)
// dictionaries: { "name": "Alice", "age": 30 }
// access: person["name"]
// sets: {1, 2, 3}
// function calls
// operators: + - * / // % **
// comparison: == != < > <= >=
// logical: and or not
// membership: in, not in
// identity: is, is not
// if, elif, else
// ternary expression: x if b else y
// loops: while, for, break, continue, pass
// def functions
// local vs global scope; nonlocal
// comprehensions (advanced)
// classes
// exceptions: try / except; finally; raise
// imports and modules: `import x`; `from x import y`; `import x as y`
// with x as y
// generators: yield
// decorators: def decorator(fund)
// type hints
// pattern matching
// async / await / pass
// walrus operator via :=
// slicing syntax: a[start:stop:step]
// unpacking
// lambda

pub fn sample_types() -> Types {
    // no variants -> panic upon construction
    let dummy = StructDef {
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
                name: "file".to_string(),
                variants: vec![
                    VariantDef {
                        name: "".to_string(),
                        fields: vec![],
                    },
                    VariantDef {
                        name: "statements".to_string(),
                        fields: vec![
                            FieldDef { name: "statements".to_string(), value: CellValue::Struct(1), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "statements".to_string(),
                variants: vec![
                    VariantDef {
                        name: "statement+".to_string(),
                        fields: vec![
                            FieldDef { name: "statement".to_string(), value: CellValue::Struct(2), is_tree: false },
                            FieldDef { name: "statement*".to_string(), value: CellValue::Struct(2), is_tree: true },
                        ],
                    }
                ],
            },
            StructDef {
                name: "statement".to_string(),
                variants: vec![
                    VariantDef {
                        name: "compound_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "compound_stmt".to_string(), value: CellValue::Struct(3), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "simple_stmts".to_string(),
                        fields: vec![
                            FieldDef { name: "simple_stmts".to_string(), value: CellValue::Struct(4), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "compound_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "function_def".to_string(),
                        fields: vec![
                            FieldDef { name: "function_def".to_string(), value: CellValue::Struct(8), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "if_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "if_stmt".to_string(), value: CellValue::Struct(9), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "class_def".to_string(),
                        fields: vec![
                            FieldDef { name: "class_def".to_string(), value: CellValue::Struct(10), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "with_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "with_stmt".to_string(), value: CellValue::Struct(11), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "for_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "for_stmt".to_string(), value: CellValue::Struct(12), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "try_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "try_stmt".to_string(), value: CellValue::Struct(13), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "while_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "while_stmt".to_string(), value: CellValue::Struct(14), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "match_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "match_stmt".to_string(), value: CellValue::Struct(15), is_tree: false },
                        ],
                    },
                ]
            },
            StructDef {
                name: "simple_stmts".to_string(),
                variants: vec![
                    VariantDef {
                        name: "';'.simple_stmt+ [';'] NEWLINE".to_string(),
                        fields: vec![
                            FieldDef { name: "simple_stmt".to_string(), value: CellValue::Struct(5), is_tree: false },
                            FieldDef { name: "(';' simple_stmt)*".to_string(), value: CellValue::Struct(6), is_tree: true },
                            FieldDef { name: "[';']".to_string(), value: CellValue::Struct(7), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "simple_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "assignment".to_string(),
                        fields: vec![
                            FieldDef { name: "assignment".to_string(), value: CellValue::Struct(16), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "type_alias".to_string(),
                        fields: vec![
                            FieldDef { name: "type_alias".to_string(), value: CellValue::Struct(17), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "star_expressions".to_string(),
                        fields: vec![
                            FieldDef { name: "star_expressions".to_string(), value: CellValue::Struct(18), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "return_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "return_stmt".to_string(), value: CellValue::Struct(19), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "import_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "import_stmt".to_string(), value: CellValue::Struct(20), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "raise_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "raise_stmt".to_string(), value: CellValue::Struct(21), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "pass_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "pass_stmt".to_string(), value: CellValue::Struct(22), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "del_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "del_stmt".to_string(), value: CellValue::Struct(23), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "yield_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "yield_stmt".to_string(), value: CellValue::Struct(24), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "assert_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "assert_stmt".to_string(), value: CellValue::Struct(25), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "break_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "break_stmt".to_string(), value: CellValue::Struct(26), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "continue_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "continue_stmt".to_string(), value: CellValue::Struct(27), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "global_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "global_stmt".to_string(), value: CellValue::Struct(28), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "nonlocal_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "nonlocal_stmt".to_string(), value: CellValue::Struct(29), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "';' simple_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "';' simple_stmt".to_string(),
                        fields: vec![
                            FieldDef { name: "simple_stmt".to_string(), value: CellValue::Struct(5), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "[';']".to_string(),
                variants: vec![
                    VariantDef {
                        name: "".to_string(),
                        fields: vec![],
                    },
                    VariantDef {
                        name: "';'".to_string(),
                        fields: vec![],
                    },
                ],
            },
            // compount_stmt
            StructDef {
                name: "function_def".to_string(),
                variants: vec![
                    VariantDef {
                        name: "decorator* function_def_raw".to_string(),
                        fields: vec![
                            FieldDef { name: "decorator*".to_string(), value: CellValue::Struct(30), is_tree: true },
                            FieldDef { name: "function_def_raw".to_string(), value: CellValue::Struct(31), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "if_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "'if' named_expression ':' block [elif_stmt | else_block]".to_string(),
                        fields: vec![
                            FieldDef { name: "named_expression".to_string(), value: CellValue::Struct(32), is_tree: false },
                            FieldDef { name: "block".to_string(), value: CellValue::Struct(33), is_tree: false },
                            FieldDef { name: "[elif_stmt | else_block]".to_string(), value: CellValue::Struct(34), is_tree: false },
                        ],
                    },
                ],
            },
            StructDef {
                name: "class_def".to_string(),
                variants: vec![
                    VariantDef {
                        name: "decorator* class_def_raw".to_string(),
                        fields: vec![
                            FieldDef { name: "decorator*".to_string(), value: CellValue::Struct(30), is_tree: true },
                            FieldDef { name: "class_def_raw".to_string(), value: CellValue::Struct(37), is_tree: false },
                        ],
                    },
                ],
            },
            // with_stmt:
            //     | ['async'] 'with' '(' with_item (',' with_item)* ','? ')' ':' block
            //     | ['async'] 'with' with_item (',' with_item)* ':' block
            //
            // requires:
            // - ['async']
            // - with_item
            // - (',' with_item)
            // ','?
            StructDef {
                name: "with_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "['async'] 'with' '(' ','.with_item+ ','? ')' ':' block".to_string(),
                        fields: vec![
                            FieldDef { name: "['async']".to_string(), value: CellValue::Struct(38), is_tree: false },
                            FieldDef { name: "with_item".to_string(), value: CellValue::Struct(39), is_tree: false },
                            FieldDef { name: "(',' with_item)".to_string(), value: CellValue::Struct(40), is_tree: true },
                            FieldDef { name: "','?".to_string(), value: CellValue::Struct(41), is_tree: false },
                            FieldDef { name: "block".to_string(), value: CellValue::Struct(33), is_tree: false },
                        ],
                    },
                    VariantDef {
                        name: "['async'] 'with' ','.with_item+ ':' block".to_string(),
                        fields: vec![
                            FieldDef { name: "['async']".to_string(), value: CellValue::Struct(38), is_tree: false },
                            FieldDef { name: "with_item".to_string(), value: CellValue::Struct(39), is_tree: false },
                            FieldDef { name: "(',' with_item)".to_string(), value: CellValue::Struct(40), is_tree: true },
                            FieldDef { name: "block".to_string(), value: CellValue::Struct(33), is_tree: false },
                        ],
                    },
                ],
            },
            // for_stmt
            // ['async'] 'for' star_targets 'in' star_expressions ':' block [else_block]
            //
            // needed new:
            // - star_targets
            // - star_expressions
            // - [else_block]
            StructDef {
                name: "for_stmt".to_string(),
                variants: vec![
                    VariantDef {
                        name: "['async'] 'for' star_targets 'in' star_expressions ':' block [else_block]".to_string(),
                        fields: vec![
                            FieldDef { name: "['async']".to_string(), value: CellValue::Struct(38), is_tree: false },
                            FieldDef { name: "star_targets".to_string(), value: CellValue::Struct(42), is_tree: false },
                            FieldDef { name: "star_expressions".to_string(), value: CellValue::Struct(43), is_tree: false },
                            FieldDef { name: "block".to_string(), value: CellValue::Struct(33), is_tree: false },
                            FieldDef { name: "[else_block]".to_string(), value: CellValue::Struct(44), is_tree: false },
                        ],
                    },
                ],
            },
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            // simple_stmt
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            dummy.clone(),
            // decorator, 30
            dummy.clone(),
            // function_def_raw, 31
            dummy.clone(),
            // named_expression, 32
            dummy.clone(),
            // block, 33
            dummy.clone(),
            // [elif_stmt | else_block], 34
            dummy.clone(),
            // elif_stmt, 35
            dummy.clone(),
            // else_block, 36
            dummy.clone(),
            // class_def_raw, 37
            dummy.clone(),
            // ['async'], 38
            dummy.clone(),
            // with_item, 39
            dummy.clone(),
            // (',' with_item), 40
            dummy.clone(),
            // ','?, 41
            dummy.clone(),
            // star_targets, 42
            dummy.clone(),
            // star_expressions, 43
            dummy.clone(),
            // [else_block], 44
            dummy.clone(),
        ],
        rim: FieldDef {
            name: "file".to_string(),
            value: CellValue::Struct(0),
            is_tree: false,
        },
    }
}


pub const STATIC_BUILDER: StaticBuilder = StaticBuilder {
    root: "",
    data: &[
        &[
            (&[], ""),
            (&[("", None)], ""),
        ],
        &[
            (&[("", None), ("", Some(""))], ""),
        ],
        &[
            (&[("", None)], ""),
            (&[("", None)], ""),
        ],
        &[
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
        ],
        &[
            (&[("", None), ("", Some("")), ("", None)], "\n"),
        ],
        &[
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
            (&[("", None)], ""),
        ],
        &[
            (&[(";", None)], ""),
        ],
        &[
            (&[], ""),
            (&[], ";"),
        ],
        // compount_stmt
        &[
            (&[("", Some("")), ("", None)], ""),
        ],
        &[
            (&[("if ", None), (": ", None), ("", None)], ""),
        ],
        &[
            (&[("", Some("")), ("", None)], ""),
        ],
        &[
            (&[("", None), (" with (", None), ("", Some("")), ("", None), ("): ", None)], ""),
            (&[("", None), (" with ", None), ("", Some("")), (": ", None)], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        // simple_stmt
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        &[
            (&[], ""),
        ],
        // decorator
        &[
            (&[], ""),
        ],
        // function_def_raw
        &[
            (&[], ""),
        ],
        // named_expression
        &[
            (&[], ""),
        ],
        // block
        &[
            (&[], ""),
        ],
        // [elif_stmt | else_block]
        &[
            (&[], ""),
        ],
        // elif_stmt
        &[
            (&[], ""),
        ],
        // else_block
        &[
            (&[], ""),
        ],
        // class_def_raw
        &[
            (&[], ""),
        ],
        // ['async']
        &[
            (&[], ""),
        ],
        // with_item
        &[
            (&[], ""),
        ],
        // (',' with_item)
        &[
            (&[], ""),
        ],
        // ','?
        &[
            (&[], ""),
        ],
        // star_targets
        &[
            (&[], ""),
        ],
        // star_expressions
        &[
            (&[], ""),
        ],
        // [else_block]
        &[
            (&[], ""),
        ],
    ],
};