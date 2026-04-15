//! Text projections: translate AIRL IR into human-readable source code.
//!
//! Supports TypeScript and Python as target languages.

use airl_ir::module::{FuncDef, Module};
use airl_ir::node::{BinOpKind, LiteralValue, MatchArm, Node, Pattern, UnaryOpKind};
use airl_ir::types::Type;

/// Supported projection languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Python,
}

impl Language {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "typescript" | "ts" => Some(Language::TypeScript),
            "python" | "py" => Some(Language::Python),
            _ => None,
        }
    }
}

/// Project an entire module to source code in the given language.
pub fn project_module(module: &Module, lang: Language) -> String {
    let mut out = String::new();

    match lang {
        Language::TypeScript => project_module_ts(module, &mut out),
        Language::Python => project_module_py(module, &mut out),
    }

    out
}

// ---------------------------------------------------------------------------
// TypeScript projection
// ---------------------------------------------------------------------------

fn project_module_ts(module: &Module, out: &mut String) {
    // Module metadata header
    let meta = &module.module.metadata;
    if !meta.description.is_empty() {
        out.push_str(&format!("// {}\n", meta.description));
    }
    out.push_str(&format!("// Module: {} v{}\n\n", module.module.name, meta.version));

    // Imports
    for import in &module.module.imports {
        let items = import.items.join(", ");
        let mod_path = import.module.replace("::", "/");
        out.push_str(&format!("import {{ {items} }} from \"{mod_path}\";\n"));
    }
    if !module.module.imports.is_empty() {
        out.push('\n');
    }

    // Functions
    for (i, func) in module.functions().iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        project_func_ts(func, out, 0);
    }
}

fn project_func_ts(func: &FuncDef, out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_to_ts(&p.param_type)))
        .collect();
    let ret = type_to_ts(&func.returns);

    // Effect annotation as JSDoc
    if !func.effects.is_empty() && !func.is_pure() {
        let effects: Vec<String> = func.effects.iter().map(|e| e.to_effect_str()).collect();
        out.push_str(&format!("{pad}/** @effects {{{}}} */\n", effects.join(", ")));
    }

    out.push_str(&format!(
        "{pad}function {}({}): {} {{\n",
        func.name,
        params.join(", "),
        ret
    ));

    project_body_ts(&func.body, out, indent + 1, true);
    out.push_str(&format!("{pad}}}\n"));
}

fn project_body_ts(node: &Node, out: &mut String, indent: usize, is_return_position: bool) {
    let pad = "  ".repeat(indent);

    match node {
        Node::Block {
            statements, result, ..
        } => {
            for stmt in statements {
                project_body_ts(stmt, out, indent, false);
            }
            project_body_ts(result, out, indent, is_return_position);
        }

        Node::Let {
            name, value, body, ..
        } => {
            out.push_str(&format!("{pad}const {name} = {};\n", expr_to_ts(value)));
            project_body_ts(body, out, indent, is_return_position);
        }

        Node::If {
            cond,
            then_branch,
            else_branch,
            node_type,
            ..
        } => {
            if is_return_position && !matches!(node_type, Type::Unit) {
                // Expression-level if → ternary or return-based
                out.push_str(&format!("{pad}if ({}) {{\n", expr_to_ts(cond)));
                project_body_ts(then_branch, out, indent + 1, true);
                out.push_str(&format!("{pad}}} else {{\n"));
                project_body_ts(else_branch, out, indent + 1, true);
                out.push_str(&format!("{pad}}}\n"));
            } else {
                out.push_str(&format!("{pad}if ({}) {{\n", expr_to_ts(cond)));
                project_body_ts(then_branch, out, indent + 1, false);
                out.push_str(&format!("{pad}}} else {{\n"));
                project_body_ts(else_branch, out, indent + 1, false);
                out.push_str(&format!("{pad}}}\n"));
            }
        }

        Node::Loop { body, .. } => {
            out.push_str(&format!("{pad}while (true) {{\n"));
            project_body_ts(body, out, indent + 1, false);
            out.push_str(&format!("{pad}}}\n"));
        }

        Node::Match {
            scrutinee, arms, ..
        } => {
            out.push_str(&format!("{pad}switch ({}) {{\n", expr_to_ts(scrutinee)));
            for arm in arms {
                project_match_arm_ts(arm, out, indent + 1, is_return_position);
            }
            out.push_str(&format!("{pad}}}\n"));
        }

        Node::Literal {
            value: LiteralValue::Unit,
            ..
        } => {
            // Skip unit literals at the end of blocks
        }

        _ => {
            let expr = expr_to_ts(node);
            if is_return_position {
                out.push_str(&format!("{pad}return {expr};\n"));
            } else {
                out.push_str(&format!("{pad}{expr};\n"));
            }
        }
    }
}

fn project_match_arm_ts(arm: &MatchArm, out: &mut String, indent: usize, is_return_pos: bool) {
    let pad = "  ".repeat(indent);
    match &arm.pattern {
        Pattern::Literal { value } => {
            out.push_str(&format!("{pad}case {}:\n", literal_to_ts(value)));
        }
        Pattern::Variable { name } => {
            out.push_str(&format!("{pad}default: // bind {name}\n"));
        }
        Pattern::Wildcard => {
            out.push_str(&format!("{pad}default:\n"));
        }
    }
    let inner_pad = "  ".repeat(indent + 1);
    let expr = expr_to_ts(&arm.body);
    if is_return_pos {
        out.push_str(&format!("{inner_pad}return {expr};\n"));
    } else {
        out.push_str(&format!("{inner_pad}{expr};\n{inner_pad}break;\n"));
    }
}

fn expr_to_ts(node: &Node) -> String {
    match node {
        Node::Literal { value, .. } => literal_to_ts(value),

        Node::Param { name, .. } => name.clone(),

        Node::Let {
            name, value, body, ..
        } => {
            // Inline let as IIFE when used as expression
            format!(
                "(() => {{ const {name} = {}; return {}; }})()",
                expr_to_ts(value),
                expr_to_ts(body)
            )
        }

        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            format!(
                "({} ? {} : {})",
                expr_to_ts(cond),
                expr_to_ts(then_branch),
                expr_to_ts(else_branch)
            )
        }

        Node::Call { target, args, .. } => {
            let func_name = builtin_to_ts(target);
            let arg_strs: Vec<String> = args.iter().map(expr_to_ts).collect();
            format!("{func_name}({})", arg_strs.join(", "))
        }

        Node::Return { value, .. } => expr_to_ts(value),

        Node::BinOp { op, lhs, rhs, .. } => {
            let op_str = binop_to_ts(op);
            format!("({} {op_str} {})", expr_to_ts(lhs), expr_to_ts(rhs))
        }

        Node::UnaryOp { op, operand, .. } => {
            let op_str = match op {
                UnaryOpKind::Neg => "-",
                UnaryOpKind::Not => "!",
                UnaryOpKind::BitNot => "~",
            };
            format!("{op_str}{}", expr_to_ts(operand))
        }

        Node::Block {
            statements, result, ..
        } => {
            if statements.is_empty() {
                expr_to_ts(result)
            } else {
                // Wrap in IIFE for expression context
                let mut inner = String::new();
                for stmt in statements {
                    inner.push_str(&format!("{}; ", expr_to_ts(stmt)));
                }
                inner.push_str(&format!("return {};", expr_to_ts(result)));
                format!("(() => {{ {inner} }})()")
            }
        }

        Node::ArrayLiteral { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(expr_to_ts).collect();
            format!("[{}]", elems.join(", "))
        }

        Node::IndexAccess { array, index, .. } => {
            format!("{}[{}]", expr_to_ts(array), expr_to_ts(index))
        }

        Node::StructLiteral { fields, .. } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(name, val)| format!("{name}: {}", expr_to_ts(val)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }

        Node::FieldAccess { object, field, .. } => {
            format!("{}.{field}", expr_to_ts(object))
        }

        Node::Match {
            scrutinee, arms, ..
        } => {
            // Inline match as chained ternaries
            let mut result = String::new();
            let scrut = expr_to_ts(scrutinee);
            for (i, arm) in arms.iter().enumerate() {
                if i > 0 {
                    result.push_str(" : ");
                }
                match &arm.pattern {
                    Pattern::Literal { value } => {
                        result.push_str(&format!(
                            "({scrut} === {} ? {}",
                            literal_to_ts(value),
                            expr_to_ts(&arm.body)
                        ));
                    }
                    Pattern::Wildcard | Pattern::Variable { .. } => {
                        result.push_str(&expr_to_ts(&arm.body));
                    }
                }
            }
            // Close parens
            for arm in arms.iter() {
                if matches!(arm.pattern, Pattern::Literal { .. }) {
                    result.push(')');
                }
            }
            result
        }

        Node::Loop { .. } => "/* loop */".to_string(),
        Node::Error { message, .. } => format!("/* ERROR: {message} */"),
    }
}

fn literal_to_ts(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Integer(i) => i.to_string(),
        LiteralValue::Float(f) => f.to_string(),
        LiteralValue::Boolean(b) => b.to_string(),
        LiteralValue::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        LiteralValue::Unit => "undefined".to_string(),
    }
}

fn binop_to_ts(op: &BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "/",
        BinOpKind::Mod => "%",
        BinOpKind::Eq => "===",
        BinOpKind::Neq => "!==",
        BinOpKind::Lt => "<",
        BinOpKind::Lte => "<=",
        BinOpKind::Gt => ">",
        BinOpKind::Gte => ">=",
        BinOpKind::And => "&&",
        BinOpKind::Or => "||",
        BinOpKind::BitAnd => "&",
        BinOpKind::BitOr => "|",
        BinOpKind::BitXor => "^",
        BinOpKind::Shl => "<<",
        BinOpKind::Shr => ">>",
    }
}

fn builtin_to_ts(target: &str) -> String {
    match target {
        "std::io::println" => "console.log".to_string(),
        "std::io::print" => "process.stdout.write".to_string(),
        "std::io::eprintln" => "console.error".to_string(),
        "std::io::read_line" => "readline".to_string(),
        "std::string::len" => "str_len".to_string(),
        "std::string::concat" => "str_concat".to_string(),
        "std::string::contains" => "str_contains".to_string(),
        "std::string::split" => "str_split".to_string(),
        "std::string::from_i64" => "String".to_string(),
        "std::string::to_i64" => "parseInt".to_string(),
        "std::string::starts_with" => "str_startsWith".to_string(),
        "std::string::ends_with" => "str_endsWith".to_string(),
        "std::string::trim" => "str_trim".to_string(),
        "std::string::to_uppercase" => "str_toUpperCase".to_string(),
        "std::string::to_lowercase" => "str_toLowerCase".to_string(),
        "std::string::replace" => "str_replace".to_string(),
        "std::math::abs" => "Math.abs".to_string(),
        "std::math::max" => "Math.max".to_string(),
        "std::math::min" => "Math.min".to_string(),
        "std::math::pow" => "Math.pow".to_string(),
        "std::math::sqrt" => "Math.sqrt".to_string(),
        "std::math::floor" => "Math.floor".to_string(),
        "std::math::ceil" => "Math.ceil".to_string(),
        "std::array::len" => "arr_len".to_string(),
        "std::array::push" => "arr_push".to_string(),
        "std::array::get" => "arr_get".to_string(),
        "std::array::slice" => "arr_slice".to_string(),
        "std::array::contains" => "arr_contains".to_string(),
        "std::array::reverse" => "arr_reverse".to_string(),
        "std::array::join" => "arr_join".to_string(),
        "std::array::range" => "arr_range".to_string(),
        "std::fmt::format" => "fmt_format".to_string(),
        "std::env::args" => "process.argv".to_string(),
        other => other.replace("::", "_"),
    }
}

fn type_to_ts(ty: &Type) -> String {
    match ty {
        Type::Unit => "void".to_string(),
        Type::Bool => "boolean".to_string(),
        Type::I8 | Type::I16 | Type::I32 | Type::I64
        | Type::U8 | Type::U16 | Type::U32 | Type::U64
        | Type::F32 | Type::F64 => "number".to_string(),
        Type::String => "string".to_string(),
        Type::Bytes => "Uint8Array".to_string(),
        Type::Array { element } => format!("{}[]", type_to_ts(element)),
        Type::Tuple { elements } => {
            let inner: Vec<String> = elements.iter().map(type_to_ts).collect();
            format!("[{}]", inner.join(", "))
        }
        Type::Optional { inner } => format!("{} | undefined", type_to_ts(inner)),
        Type::Result { ok, err } => format!("Result<{}, {}>", type_to_ts(ok), type_to_ts(err)),
        Type::Struct { name, .. } => name.0.clone(),
        Type::Enum { name, .. } => name.0.clone(),
        Type::Function { params, returns, .. } => {
            let p: Vec<String> = params.iter().enumerate()
                .map(|(i, t)| format!("arg{i}: {}", type_to_ts(t)))
                .collect();
            format!("({}) => {}", p.join(", "), type_to_ts(returns))
        }
        Type::Reference { inner, .. } => type_to_ts(inner),
        Type::Named(id) => id.0.clone(),
        Type::TypeParam { name, .. } => name.0.clone(),
        Type::Generic { base, args } => {
            let a: Vec<String> = args.iter().map(type_to_ts).collect();
            format!("{}<{}>", type_to_ts(base), a.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// Python projection
// ---------------------------------------------------------------------------

fn project_module_py(module: &Module, out: &mut String) {
    // Module docstring
    let meta = &module.module.metadata;
    if !meta.description.is_empty() {
        out.push_str(&format!("\"\"\"{}\"\"\"", meta.description));
        out.push('\n');
    }
    out.push_str(&format!("# Module: {} v{}\n\n", module.module.name, meta.version));

    // Imports
    for import in &module.module.imports {
        let items = import.items.join(", ");
        let mod_path = import.module.replace("::", ".");
        out.push_str(&format!("from {mod_path} import {items}\n"));
    }
    if !module.module.imports.is_empty() {
        out.push('\n');
    }

    // Functions
    for (i, func) in module.functions().iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        project_func_py(func, out, 0);
    }

    // If there's a main function, add the idiom
    if module.find_function("main").is_some() {
        out.push_str("\nif __name__ == \"__main__\":\n    main()\n");
    }
}

fn project_func_py(func: &FuncDef, out: &mut String, indent: usize) {
    let pad = "    ".repeat(indent);
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_to_py(&p.param_type)))
        .collect();
    let ret = type_to_py(&func.returns);

    out.push_str(&format!(
        "{pad}def {}({}) -> {ret}:\n",
        func.name,
        params.join(", "),
    ));

    let mut body_out = String::new();
    project_body_py(&func.body, &mut body_out, indent + 1, true);

    if body_out.trim().is_empty() {
        out.push_str(&format!("{}    pass\n", pad));
    } else {
        out.push_str(&body_out);
    }
}

fn project_body_py(node: &Node, out: &mut String, indent: usize, is_return_position: bool) {
    let pad = "    ".repeat(indent);

    match node {
        Node::Block {
            statements, result, ..
        } => {
            for stmt in statements {
                project_body_py(stmt, out, indent, false);
            }
            project_body_py(result, out, indent, is_return_position);
        }

        Node::Let {
            name, value, body, ..
        } => {
            out.push_str(&format!("{pad}{name} = {}\n", expr_to_py(value)));
            project_body_py(body, out, indent, is_return_position);
        }

        Node::If {
            cond,
            then_branch,
            else_branch,
            node_type,
            ..
        } => {
            if is_return_position && !matches!(node_type, Type::Unit) {
                out.push_str(&format!("{pad}if {}:\n", expr_to_py(cond)));
                project_body_py(then_branch, out, indent + 1, true);
                out.push_str(&format!("{pad}else:\n"));
                project_body_py(else_branch, out, indent + 1, true);
            } else {
                out.push_str(&format!("{pad}if {}:\n", expr_to_py(cond)));
                project_body_py(then_branch, out, indent + 1, false);
                out.push_str(&format!("{pad}else:\n"));
                project_body_py(else_branch, out, indent + 1, false);
            }
        }

        Node::Loop { body, .. } => {
            out.push_str(&format!("{pad}while True:\n"));
            project_body_py(body, out, indent + 1, false);
        }

        Node::Match {
            scrutinee, arms, ..
        } => {
            out.push_str(&format!("{pad}match {}:\n", expr_to_py(scrutinee)));
            for arm in arms {
                project_match_arm_py(arm, out, indent + 1, is_return_position);
            }
        }

        Node::Literal {
            value: LiteralValue::Unit,
            ..
        } => {
            // Skip unit literals at the end of blocks
        }

        _ => {
            let expr = expr_to_py(node);
            if is_return_position {
                out.push_str(&format!("{pad}return {expr}\n"));
            } else {
                out.push_str(&format!("{pad}{expr}\n"));
            }
        }
    }
}

fn project_match_arm_py(arm: &MatchArm, out: &mut String, indent: usize, is_return_pos: bool) {
    let pad = "    ".repeat(indent);
    let inner_pad = "    ".repeat(indent + 1);

    match &arm.pattern {
        Pattern::Literal { value } => {
            out.push_str(&format!("{pad}case {}:\n", literal_to_py(value)));
        }
        Pattern::Variable { name } => {
            out.push_str(&format!("{pad}case {name}:\n"));
        }
        Pattern::Wildcard => {
            out.push_str(&format!("{pad}case _:\n"));
        }
    }

    let expr = expr_to_py(&arm.body);
    if is_return_pos {
        out.push_str(&format!("{inner_pad}return {expr}\n"));
    } else {
        out.push_str(&format!("{inner_pad}{expr}\n"));
    }
}

fn expr_to_py(node: &Node) -> String {
    match node {
        Node::Literal { value, .. } => literal_to_py(value),

        Node::Param { name, .. } => name.clone(),

        Node::Let {
            name, value, body, ..
        } => {
            // Python doesn't have inline let; approximate with walrus
            format!(
                "({name} := {}, {})[1]",
                expr_to_py(value),
                expr_to_py(body)
            )
        }

        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            format!(
                "({} if {} else {})",
                expr_to_py(then_branch),
                expr_to_py(cond),
                expr_to_py(else_branch)
            )
        }

        Node::Call { target, args, .. } => {
            let func_name = builtin_to_py(target);
            let arg_strs: Vec<String> = args.iter().map(expr_to_py).collect();
            format!("{func_name}({})", arg_strs.join(", "))
        }

        Node::Return { value, .. } => expr_to_py(value),

        Node::BinOp { op, lhs, rhs, .. } => {
            let op_str = binop_to_py(op);
            format!("({} {op_str} {})", expr_to_py(lhs), expr_to_py(rhs))
        }

        Node::UnaryOp { op, operand, .. } => {
            let op_str = match op {
                UnaryOpKind::Neg => "-",
                UnaryOpKind::Not => "not ",
                UnaryOpKind::BitNot => "~",
            };
            format!("{op_str}{}", expr_to_py(operand))
        }

        Node::Block {
            statements, result, ..
        } => {
            if statements.is_empty() {
                expr_to_py(result)
            } else {
                // Can't easily inline blocks in Python; just emit the result
                expr_to_py(result)
            }
        }

        Node::ArrayLiteral { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(expr_to_py).collect();
            format!("[{}]", elems.join(", "))
        }

        Node::IndexAccess { array, index, .. } => {
            format!("{}[{}]", expr_to_py(array), expr_to_py(index))
        }

        Node::StructLiteral { fields, .. } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(name, val)| format!("\"{name}\": {}", expr_to_py(val)))
                .collect();
            format!("{{{}}}", field_strs.join(", "))
        }

        Node::FieldAccess { object, field, .. } => {
            format!("{}[\"{field}\"]", expr_to_py(object))
        }

        Node::Match {
            scrutinee, arms, ..
        } => {
            // Inline as chained ternaries (Python conditional expressions)
            let scrut = expr_to_py(scrutinee);
            let mut parts = Vec::new();
            let mut default = None;
            for arm in arms {
                match &arm.pattern {
                    Pattern::Literal { value } => {
                        parts.push(format!(
                            "{} if {scrut} == {}",
                            expr_to_py(&arm.body),
                            literal_to_py(value)
                        ));
                    }
                    Pattern::Wildcard | Pattern::Variable { .. } => {
                        default = Some(expr_to_py(&arm.body));
                    }
                }
            }
            if let Some(def) = default {
                parts.push(def);
            }
            parts.join(" else ")
        }

        Node::Loop { .. } => "None  # loop".to_string(),
        Node::Error { message, .. } => format!("raise RuntimeError(\"{message}\")"),
    }
}

fn literal_to_py(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Integer(i) => i.to_string(),
        LiteralValue::Float(f) => f.to_string(),
        LiteralValue::Boolean(true) => "True".to_string(),
        LiteralValue::Boolean(false) => "False".to_string(),
        LiteralValue::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        LiteralValue::Unit => "None".to_string(),
    }
}

fn binop_to_py(op: &BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "//",
        BinOpKind::Mod => "%",
        BinOpKind::Eq => "==",
        BinOpKind::Neq => "!=",
        BinOpKind::Lt => "<",
        BinOpKind::Lte => "<=",
        BinOpKind::Gt => ">",
        BinOpKind::Gte => ">=",
        BinOpKind::And => "and",
        BinOpKind::Or => "or",
        BinOpKind::BitAnd => "&",
        BinOpKind::BitOr => "|",
        BinOpKind::BitXor => "^",
        BinOpKind::Shl => "<<",
        BinOpKind::Shr => ">>",
    }
}

fn builtin_to_py(target: &str) -> String {
    match target {
        "std::io::println" => "print".to_string(),
        "std::io::print" => "print".to_string(),
        "std::io::eprintln" => "print".to_string(),
        "std::io::read_line" => "input".to_string(),
        "std::string::len" => "len".to_string(),
        "std::string::concat" => "str_concat".to_string(),
        "std::string::contains" => "str_contains".to_string(),
        "std::string::split" => "str_split".to_string(),
        "std::string::from_i64" => "str".to_string(),
        "std::string::to_i64" => "int".to_string(),
        "std::string::starts_with" => "str_startswith".to_string(),
        "std::string::ends_with" => "str_endswith".to_string(),
        "std::string::trim" => "str_strip".to_string(),
        "std::string::to_uppercase" => "str_upper".to_string(),
        "std::string::to_lowercase" => "str_lower".to_string(),
        "std::string::replace" => "str_replace".to_string(),
        "std::math::abs" => "abs".to_string(),
        "std::math::max" => "max".to_string(),
        "std::math::min" => "min".to_string(),
        "std::math::pow" => "pow".to_string(),
        "std::math::sqrt" => "math.sqrt".to_string(),
        "std::math::floor" => "math.floor".to_string(),
        "std::math::ceil" => "math.ceil".to_string(),
        "std::array::len" => "len".to_string(),
        "std::array::push" => "arr_push".to_string(),
        "std::array::get" => "arr_get".to_string(),
        "std::array::slice" => "arr_slice".to_string(),
        "std::array::contains" => "arr_contains".to_string(),
        "std::array::reverse" => "arr_reverse".to_string(),
        "std::array::join" => "arr_join".to_string(),
        "std::array::range" => "range".to_string(),
        "std::fmt::format" => "fmt_format".to_string(),
        "std::env::args" => "sys.argv".to_string(),
        other => other.replace("::", "_"),
    }
}

fn type_to_py(ty: &Type) -> String {
    match ty {
        Type::Unit => "None".to_string(),
        Type::Bool => "bool".to_string(),
        Type::I8 | Type::I16 | Type::I32 | Type::I64
        | Type::U8 | Type::U16 | Type::U32 | Type::U64 => "int".to_string(),
        Type::F32 | Type::F64 => "float".to_string(),
        Type::String => "str".to_string(),
        Type::Bytes => "bytes".to_string(),
        Type::Array { element } => format!("list[{}]", type_to_py(element)),
        Type::Tuple { elements } => {
            let inner: Vec<String> = elements.iter().map(type_to_py).collect();
            format!("tuple[{}]", inner.join(", "))
        }
        Type::Optional { inner } => format!("{} | None", type_to_py(inner)),
        Type::Result { ok, .. } => type_to_py(ok),
        Type::Struct { name, .. } => name.0.clone(),
        Type::Enum { name, .. } => name.0.clone(),
        Type::Function { params, returns, .. } => {
            let p: Vec<String> = params.iter().map(type_to_py).collect();
            format!("Callable[[{}], {}]", p.join(", "), type_to_py(returns))
        }
        Type::Reference { inner, .. } => type_to_py(inner),
        Type::Named(id) => id.0.clone(),
        Type::TypeParam { name, .. } => name.0.clone(),
        Type::Generic { base, args } => {
            let a: Vec<String> = args.iter().map(type_to_py).collect();
            format!("{}[{}]", type_to_py(base), a.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airl_ir::Module;

    fn load_module(json: &str) -> Module {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn test_hello_world_typescript() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [{"module": "std::io", "items": ["println"]}],
                "exports": [], "types": [],
                "functions": [{
                    "id": "f_main", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n1", "kind": "Call", "type": "Unit",
                        "target": "std::io::println",
                        "args": [{"id": "n2", "kind": "Literal", "type": "String", "value": "hello world"}]
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let ts = project_module(&module, Language::TypeScript);
        assert!(ts.contains("console.log(\"hello world\")"));
        assert!(ts.contains("function main()"));
    }

    #[test]
    fn test_hello_world_python() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [{"module": "std::io", "items": ["println"]}],
                "exports": [], "types": [],
                "functions": [{
                    "id": "f_main", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n1", "kind": "Call", "type": "Unit",
                        "target": "std::io::println",
                        "args": [{"id": "n2", "kind": "Literal", "type": "String", "value": "hello world"}]
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let py = project_module(&module, Language::Python);
        assert!(py.contains("print(\"hello world\")"));
        assert!(py.contains("def main()"));
        assert!(py.contains("if __name__"));
    }

    #[test]
    fn test_fibonacci_typescript() {
        let json = std::fs::read_to_string("../../../examples/fibonacci.airl.json")
            .unwrap_or_else(|_| {
                // Fallback for test environments
                include_str!("../../../examples/fibonacci.airl.json").to_string()
            });
        let module = load_module(&json);
        let ts = project_module(&module, Language::TypeScript);
        assert!(ts.contains("function fib("));
        assert!(ts.contains("function main("));
        assert!(ts.contains("console.log"));
    }

    #[test]
    fn test_fibonacci_python() {
        let json = std::fs::read_to_string("../../../examples/fibonacci.airl.json")
            .unwrap_or_else(|_| {
                include_str!("../../../examples/fibonacci.airl.json").to_string()
            });
        let module = load_module(&json);
        let py = project_module(&module, Language::Python);
        assert!(py.contains("def fib("));
        assert!(py.contains("def main("));
        assert!(py.contains("print("));
    }

    #[test]
    fn test_let_binding_typescript() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [], "exports": [], "types": [],
                "functions": [{
                    "id": "f_main", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n1", "kind": "Let", "type": "Unit", "name": "x",
                        "value": {"id": "n2", "kind": "Literal", "type": "I64", "value": 42},
                        "body": {"id": "n3", "kind": "Call", "type": "Unit",
                            "target": "std::io::println",
                            "args": [{"id": "n4", "kind": "Param", "type": "I64", "name": "x", "index": 0}]
                        }
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let ts = project_module(&module, Language::TypeScript);
        assert!(ts.contains("const x = 42"));
        assert!(ts.contains("console.log(x)"));
    }

    #[test]
    fn test_if_else_typescript() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [], "exports": [], "types": [],
                "functions": [{
                    "id": "f", "name": "max_val",
                    "params": [{"name": "a", "type": "I64", "index": 0}, {"name": "b", "type": "I64", "index": 1}],
                    "returns": "I64", "effects": ["Pure"],
                    "body": {"id": "n1", "kind": "If", "type": "I64",
                        "cond": {"id": "n2", "kind": "BinOp", "type": "Bool", "op": "Gt",
                            "lhs": {"id": "n3", "kind": "Param", "type": "I64", "name": "a", "index": 0},
                            "rhs": {"id": "n4", "kind": "Param", "type": "I64", "name": "b", "index": 1}
                        },
                        "then_branch": {"id": "n5", "kind": "Param", "type": "I64", "name": "a", "index": 0},
                        "else_branch": {"id": "n6", "kind": "Param", "type": "I64", "name": "b", "index": 1}
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let ts = project_module(&module, Language::TypeScript);
        assert!(ts.contains("function max_val(a: number, b: number): number"));
        assert!(ts.contains("if ("));
        assert!(ts.contains("return a"));
        assert!(ts.contains("return b"));
    }

    #[test]
    fn test_if_else_python() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [], "exports": [], "types": [],
                "functions": [{
                    "id": "f", "name": "max_val",
                    "params": [{"name": "a", "type": "I64", "index": 0}, {"name": "b", "type": "I64", "index": 1}],
                    "returns": "I64", "effects": ["Pure"],
                    "body": {"id": "n1", "kind": "If", "type": "I64",
                        "cond": {"id": "n2", "kind": "BinOp", "type": "Bool", "op": "Gt",
                            "lhs": {"id": "n3", "kind": "Param", "type": "I64", "name": "a", "index": 0},
                            "rhs": {"id": "n4", "kind": "Param", "type": "I64", "name": "b", "index": 1}
                        },
                        "then_branch": {"id": "n5", "kind": "Param", "type": "I64", "name": "a", "index": 0},
                        "else_branch": {"id": "n6", "kind": "Param", "type": "I64", "name": "b", "index": 1}
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let py = project_module(&module, Language::Python);
        assert!(py.contains("def max_val(a: int, b: int) -> int:"));
        assert!(py.contains("if "));
        assert!(py.contains("return a"));
        assert!(py.contains("return b"));
    }

    #[test]
    fn test_array_literal_typescript() {
        let json = r#"{
            "format_version": "0.1.0",
            "module": {
                "id": "m", "name": "main",
                "metadata": {"version": "1", "description": "", "author": "", "created_at": ""},
                "imports": [], "exports": [], "types": [],
                "functions": [{
                    "id": "f", "name": "main", "params": [], "returns": "Unit",
                    "effects": ["IO"],
                    "body": {"id": "n1", "kind": "Call", "type": "Unit",
                        "target": "std::io::println",
                        "args": [{"id": "n2", "kind": "ArrayLiteral", "type": "Array<I64>",
                            "elements": [
                                {"id": "n3", "kind": "Literal", "type": "I64", "value": 1},
                                {"id": "n4", "kind": "Literal", "type": "I64", "value": 2},
                                {"id": "n5", "kind": "Literal", "type": "I64", "value": 3}
                            ]
                        }]
                    }
                }]
            }
        }"#;

        let module = load_module(json);
        let ts = project_module(&module, Language::TypeScript);
        assert!(ts.contains("[1, 2, 3]"));
    }
}
