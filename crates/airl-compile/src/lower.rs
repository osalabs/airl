//! IR → Cranelift JIT lowering.
//!
//! Translates AIRL IR nodes into Cranelift IR, JIT-compiles them,
//! and executes the result.
//!
//! Strings are represented as i64 handles indexing into a global string table.
//! Runtime helper functions operate on these handles to perform string operations.

use std::collections::HashMap;
use std::sync::Mutex;

use airl_ir::module::{FuncDef, Module};
use airl_ir::node::{BinOpKind, LiteralValue, Node, UnaryOpKind};
use airl_ir::types::Type;

use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types as cl_types;
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, Signature, UserFuncName};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module as CraneliftModule};

use crate::CompileError;

// ---------------------------------------------------------------------------
// Runtime: stdout buffer and dynamic string table
// ---------------------------------------------------------------------------

static JIT_STDOUT: Mutex<Option<String>> = Mutex::new(None);
static JIT_STRINGS: Mutex<Option<Vec<String>>> = Mutex::new(None);

// --- I/O runtime functions ---

extern "C" fn airl_print_i64(val: i64) {
    let mut lock = JIT_STDOUT.lock().unwrap();
    if let Some(buf) = lock.as_mut() {
        buf.push_str(&val.to_string());
        buf.push('\n');
    }
}

extern "C" fn airl_print_str(ptr: *const u8, len: i64) {
    let mut lock = JIT_STDOUT.lock().unwrap();
    if let Some(buf) = lock.as_mut() {
        let slice = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        if let Ok(s) = std::str::from_utf8(slice) {
            buf.push_str(s);
        }
        buf.push('\n');
    }
}

/// Print a string handle followed by newline.
extern "C" fn airl_println_handle(handle: i64) {
    let lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_ref() {
        if let Some(s) = table.get(handle as usize) {
            let mut out = JIT_STDOUT.lock().unwrap();
            if let Some(buf) = out.as_mut() {
                buf.push_str(s);
                buf.push('\n');
            }
        }
    }
}

/// Print a string handle without newline.
extern "C" fn airl_print_handle(handle: i64) {
    let lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_ref() {
        if let Some(s) = table.get(handle as usize) {
            let mut out = JIT_STDOUT.lock().unwrap();
            if let Some(buf) = out.as_mut() {
                buf.push_str(s);
            }
        }
    }
}

// --- String runtime functions ---

/// Concatenate two string handles, return new handle.
extern "C" fn airl_str_concat(a: i64, b: i64) -> i64 {
    let mut lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_mut() {
        let sa = table.get(a as usize).cloned().unwrap_or_default();
        let sb = table.get(b as usize).cloned().unwrap_or_default();
        let result = format!("{sa}{sb}");
        let idx = table.len();
        table.push(result);
        idx as i64
    } else {
        0
    }
}

/// Get length of a string handle.
extern "C" fn airl_str_len(handle: i64) -> i64 {
    let lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_ref() {
        table.get(handle as usize).map(|s| s.len() as i64).unwrap_or(0)
    } else {
        0
    }
}

/// Convert i64 to string, return handle.
extern "C" fn airl_str_from_i64(val: i64) -> i64 {
    let mut lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_mut() {
        let s = val.to_string();
        let idx = table.len();
        table.push(s);
        idx as i64
    } else {
        0
    }
}

/// Check if string contains substring.
extern "C" fn airl_str_contains(haystack: i64, needle: i64) -> i64 {
    let lock = JIT_STRINGS.lock().unwrap();
    if let Some(table) = lock.as_ref() {
        let h = table.get(haystack as usize).map(|s| s.as_str()).unwrap_or("");
        let n = table.get(needle as usize).map(|s| s.as_str()).unwrap_or("");
        if h.contains(n) { 1 } else { 0 }
    } else {
        0
    }
}

// --- Math runtime functions ---

extern "C" fn airl_math_abs(val: i64) -> i64 {
    val.abs()
}

extern "C" fn airl_math_max(a: i64, b: i64) -> i64 {
    a.max(b)
}

extern "C" fn airl_math_min(a: i64, b: i64) -> i64 {
    a.min(b)
}

extern "C" fn airl_math_pow(base: i64, exp: i64) -> i64 {
    base.wrapping_pow(exp as u32)
}

// ---------------------------------------------------------------------------
// JIT compilation entry point
// ---------------------------------------------------------------------------

pub fn jit_compile_and_run(module: &Module) -> Result<String, CompileError> {
    module
        .find_function("main")
        .ok_or(CompileError::NoMainFunction)?;

    // Build Cranelift JIT module
    let mut flag_builder = settings::builder();
    flag_builder.set("is_pic", "false").unwrap();
    let isa_builder =
        cranelift_native::builder().map_err(|e| CompileError::CodegenError(e.to_string()))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| CompileError::CodegenError(e.to_string()))?;

    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

    // Register all runtime functions
    jit_builder.symbol("airl_print_i64", airl_print_i64 as *const u8);
    jit_builder.symbol("airl_print_str", airl_print_str as *const u8);
    jit_builder.symbol("airl_println_handle", airl_println_handle as *const u8);
    jit_builder.symbol("airl_print_handle", airl_print_handle as *const u8);
    jit_builder.symbol("airl_str_concat", airl_str_concat as *const u8);
    jit_builder.symbol("airl_str_len", airl_str_len as *const u8);
    jit_builder.symbol("airl_str_from_i64", airl_str_from_i64 as *const u8);
    jit_builder.symbol("airl_str_contains", airl_str_contains as *const u8);
    jit_builder.symbol("airl_math_abs", airl_math_abs as *const u8);
    jit_builder.symbol("airl_math_max", airl_math_max as *const u8);
    jit_builder.symbol("airl_math_min", airl_math_min as *const u8);
    jit_builder.symbol("airl_math_pow", airl_math_pow as *const u8);

    let mut jit_module = JITModule::new(jit_builder);

    // Collect string literals for the string table
    let mut string_table: Vec<String> = Vec::new();
    for func in module.functions() {
        collect_string_literals(&func.body, &mut string_table);
    }

    // Initialize the global string table with literal strings
    {
        let mut lock = JIT_STRINGS.lock().unwrap();
        *lock = Some(string_table.clone());
    }

    // Build a compiler context, compile, then extract func_ids
    let func_ids = {
        let mut compiler = JitCompiler::new(&mut jit_module, module, &string_table)?;
        compiler.declare_all_functions()?;
        compiler.compile_all_functions()?;
        compiler.func_ids.clone()
    };

    // Finalize
    jit_module
        .finalize_definitions()
        .map_err(|e| CompileError::ModuleError(e.to_string()))?;

    // Get main function pointer
    let main_id = func_ids
        .get("main")
        .ok_or(CompileError::NoMainFunction)?;
    let main_ptr = jit_module.get_finalized_function(*main_id);

    // Set up stdout capture and call main
    {
        let mut lock = JIT_STDOUT.lock().unwrap();
        *lock = Some(String::new());
    }

    let main_fn: fn() = unsafe { std::mem::transmute(main_ptr) };
    main_fn();

    // Retrieve captured stdout
    let stdout = {
        let mut lock = JIT_STDOUT.lock().unwrap();
        lock.take().unwrap_or_default()
    };

    // Clean up string table
    {
        let mut lock = JIT_STRINGS.lock().unwrap();
        *lock = None;
    }

    Ok(stdout)
}

/// Collect all string literal values in a node tree.
fn collect_string_literals(node: &Node, table: &mut Vec<String>) {
    match node {
        Node::Literal {
            value: LiteralValue::Str(s),
            ..
        } => {
            if !table.contains(s) {
                table.push(s.clone());
            }
        }
        Node::Let { value, body, .. } => {
            collect_string_literals(value, table);
            collect_string_literals(body, table);
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_string_literals(cond, table);
            collect_string_literals(then_branch, table);
            collect_string_literals(else_branch, table);
        }
        Node::Call { args, .. } => {
            for arg in args {
                collect_string_literals(arg, table);
            }
        }
        Node::Return { value, .. } => collect_string_literals(value, table),
        Node::BinOp { lhs, rhs, .. } => {
            collect_string_literals(lhs, table);
            collect_string_literals(rhs, table);
        }
        Node::UnaryOp { operand, .. } => collect_string_literals(operand, table),
        Node::Block {
            statements, result, ..
        } => {
            for s in statements {
                collect_string_literals(s, table);
            }
            collect_string_literals(result, table);
        }
        Node::Match {
            scrutinee, arms, ..
        } => {
            collect_string_literals(scrutinee, table);
            for arm in arms {
                collect_string_literals(&arm.body, table);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// JIT Compiler struct
// ---------------------------------------------------------------------------

struct JitCompiler<'a> {
    jit_module: &'a mut JITModule,
    airl_module: &'a Module,
    string_table: &'a [String],
    func_ids: HashMap<String, FuncId>,
    // Runtime function IDs
    print_i64_id: FuncId,
    print_str_id: FuncId,
    println_handle_id: FuncId,
    print_handle_id: FuncId,
    str_concat_id: FuncId,
    str_len_id: FuncId,
    str_from_i64_id: FuncId,
    str_contains_id: FuncId,
    math_abs_id: FuncId,
    math_max_id: FuncId,
    math_min_id: FuncId,
    math_pow_id: FuncId,
    var_counter: u32,
}

impl<'a> JitCompiler<'a> {
    fn new(
        jit_module: &'a mut JITModule,
        airl_module: &'a Module,
        string_table: &'a [String],
    ) -> Result<Self, CompileError> {
        let ptr_type = jit_module.target_config().pointer_type();

        // Helper to declare a runtime function
        macro_rules! decl_rt {
            ($name:expr, [$($param:expr),*], [$($ret:expr),*]) => {{
                let mut sig = jit_module.make_signature();
                $(sig.params.push(AbiParam::new($param));)*
                $(sig.returns.push(AbiParam::new($ret));)*
                jit_module.declare_function($name, Linkage::Import, &sig)
                    .map_err(|e| CompileError::ModuleError(e.to_string()))?
            }};
        }

        let print_i64_id = decl_rt!("airl_print_i64", [cl_types::I64], []);
        let print_str_id = decl_rt!("airl_print_str", [ptr_type, cl_types::I64], []);
        let println_handle_id = decl_rt!("airl_println_handle", [cl_types::I64], []);
        let print_handle_id = decl_rt!("airl_print_handle", [cl_types::I64], []);
        let str_concat_id = decl_rt!("airl_str_concat", [cl_types::I64, cl_types::I64], [cl_types::I64]);
        let str_len_id = decl_rt!("airl_str_len", [cl_types::I64], [cl_types::I64]);
        let str_from_i64_id = decl_rt!("airl_str_from_i64", [cl_types::I64], [cl_types::I64]);
        let str_contains_id = decl_rt!("airl_str_contains", [cl_types::I64, cl_types::I64], [cl_types::I64]);
        let math_abs_id = decl_rt!("airl_math_abs", [cl_types::I64], [cl_types::I64]);
        let math_max_id = decl_rt!("airl_math_max", [cl_types::I64, cl_types::I64], [cl_types::I64]);
        let math_min_id = decl_rt!("airl_math_min", [cl_types::I64, cl_types::I64], [cl_types::I64]);
        let math_pow_id = decl_rt!("airl_math_pow", [cl_types::I64, cl_types::I64], [cl_types::I64]);

        Ok(Self {
            jit_module,
            airl_module,
            string_table,
            func_ids: HashMap::new(),
            print_i64_id,
            print_str_id,
            println_handle_id,
            print_handle_id,
            str_concat_id,
            str_len_id,
            str_from_i64_id,
            str_contains_id,
            math_abs_id,
            math_max_id,
            math_min_id,
            math_pow_id,
            var_counter: 0,
        })
    }

    fn declare_all_functions(&mut self) -> Result<(), CompileError> {
        for func in self.airl_module.functions() {
            let sig = self.build_signature(func);
            let func_id = self
                .jit_module
                .declare_function(&func.name, Linkage::Local, &sig)
                .map_err(|e| CompileError::ModuleError(e.to_string()))?;
            self.func_ids.insert(func.name.clone(), func_id);
        }
        Ok(())
    }

    fn compile_all_functions(&mut self) -> Result<(), CompileError> {
        for func_def in self.airl_module.functions() {
            self.compile_function(func_def)?;
        }
        Ok(())
    }

    fn build_signature(&self, func: &FuncDef) -> Signature {
        let call_conv = self.jit_module.isa().default_call_conv();
        let mut sig = Signature::new(call_conv);
        for param in &func.params {
            sig.params.push(AbiParam::new(self.airl_type_to_cranelift(&param.param_type)));
        }
        if !matches!(func.returns, Type::Unit) {
            sig.returns.push(AbiParam::new(self.airl_type_to_cranelift(&func.returns)));
        }
        sig
    }

    fn airl_type_to_cranelift(&self, ty: &Type) -> cranelift_codegen::ir::Type {
        match ty {
            Type::I64 | Type::I32 | Type::Bool | Type::Unit | Type::String => cl_types::I64,
            Type::F64 => cl_types::F64,
            Type::F32 => cl_types::F32,
            _ => cl_types::I64,
        }
    }

    fn compile_function(&mut self, func_def: &FuncDef) -> Result<(), CompileError> {
        let func_id = *self
            .func_ids
            .get(&func_def.name)
            .ok_or_else(|| CompileError::FunctionNotFound(func_def.name.clone()))?;

        let sig = self.build_signature(func_def);
        let mut cl_func = Function::with_name_signature(UserFuncName::default(), sig);
        let mut fb_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut cl_func, &mut fb_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        self.var_counter = 0;
        let mut var_map: HashMap<String, Variable> = HashMap::new();
        for (i, param) in func_def.params.iter().enumerate() {
            let var = Variable::new(self.next_var());
            let cl_type = self.airl_type_to_cranelift(&param.param_type);
            builder.declare_var(var, cl_type);
            let param_val = builder.block_params(entry_block)[i];
            builder.def_var(var, param_val);
            var_map.insert(param.name.clone(), var);
        }

        let result = self.lower_node(&func_def.body, &mut builder, &mut var_map)?;

        if matches!(func_def.returns, Type::Unit) {
            builder.ins().return_(&[]);
        } else {
            builder.ins().return_(&[result]);
        }

        builder.finalize();

        let mut ctx = cranelift_codegen::Context::for_function(cl_func);
        self.jit_module
            .define_function(func_id, &mut ctx)
            .map_err(|e| CompileError::CodegenError(e.to_string()))?;

        Ok(())
    }

    fn next_var(&mut self) -> usize {
        let v = self.var_counter as usize;
        self.var_counter += 1;
        v
    }

    /// Call a runtime function by its FuncId with given args, returning optional result.
    fn call_runtime(
        &mut self,
        rt_id: FuncId,
        args: &[cranelift_codegen::ir::Value],
        builder: &mut FunctionBuilder,
        has_return: bool,
    ) -> cranelift_codegen::ir::Value {
        let func_ref = self.jit_module.declare_func_in_func(rt_id, builder.func);
        let call = builder.ins().call(func_ref, args);
        if has_return {
            builder.inst_results(call)[0]
        } else {
            builder.ins().iconst(cl_types::I64, 0)
        }
    }

    fn lower_node(
        &mut self,
        node: &Node,
        builder: &mut FunctionBuilder,
        var_map: &mut HashMap<String, Variable>,
    ) -> Result<cranelift_codegen::ir::Value, CompileError> {
        match node {
            Node::Literal { value, .. } => match value {
                LiteralValue::Integer(i) => Ok(builder.ins().iconst(cl_types::I64, *i)),
                LiteralValue::Boolean(b) => {
                    Ok(builder.ins().iconst(cl_types::I64, if *b { 1 } else { 0 }))
                }
                LiteralValue::Float(f) => Ok(builder.ins().f64const(*f)),
                LiteralValue::Unit => Ok(builder.ins().iconst(cl_types::I64, 0)),
                LiteralValue::Str(s) => {
                    // Return the string table handle (index)
                    let idx = self
                        .string_table
                        .iter()
                        .position(|x| x == s)
                        .unwrap_or(0);
                    Ok(builder.ins().iconst(cl_types::I64, idx as i64))
                }
            },

            Node::Param { name, id, .. } => {
                let var = var_map.get(name).ok_or_else(|| {
                    CompileError::CodegenError(format!("undefined variable: {name} at {id}"))
                })?;
                Ok(builder.use_var(*var))
            }

            Node::Let {
                name,
                value,
                body,
                node_type,
                ..
            } => {
                let val = self.lower_node(value, builder, var_map)?;
                let var = Variable::new(self.next_var());
                let cl_type = self.airl_type_to_cranelift(node_type);
                builder.declare_var(var, cl_type);
                builder.def_var(var, val);
                var_map.insert(name.clone(), var);
                let result = self.lower_node(body, builder, var_map)?;
                var_map.remove(name);
                Ok(result)
            }

            Node::If {
                cond,
                then_branch,
                else_branch,
                node_type,
                ..
            } => {
                let cond_val = self.lower_node(cond, builder, var_map)?;
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let merge_block = builder.create_block();
                let cl_type = self.airl_type_to_cranelift(node_type);
                builder.append_block_param(merge_block, cl_type);
                builder.ins().brif(cond_val, then_block, &[], else_block, &[]);

                builder.switch_to_block(then_block);
                builder.seal_block(then_block);
                let then_val = self.lower_node(then_branch, builder, var_map)?;
                builder.ins().jump(merge_block, &[then_val]);

                builder.switch_to_block(else_block);
                builder.seal_block(else_block);
                let else_val = self.lower_node(else_branch, builder, var_map)?;
                builder.ins().jump(merge_block, &[else_val]);

                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);
                Ok(builder.block_params(merge_block)[0])
            }

            Node::Call {
                target,
                args,
                node_type,
                ..
            } => self.lower_call(target, args, node_type, builder, var_map),

            Node::Return { value, .. } => self.lower_node(value, builder, var_map),

            Node::BinOp {
                op, lhs, rhs, ..
            } => {
                let l = self.lower_node(lhs, builder, var_map)?;
                let r = self.lower_node(rhs, builder, var_map)?;
                self.lower_binop(op, l, r, builder)
            }

            Node::UnaryOp { op, operand, .. } => {
                let val = self.lower_node(operand, builder, var_map)?;
                match op {
                    UnaryOpKind::Neg => Ok(builder.ins().ineg(val)),
                    UnaryOpKind::Not => {
                        let one = builder.ins().iconst(cl_types::I64, 1);
                        Ok(builder.ins().bxor(val, one))
                    }
                    UnaryOpKind::BitNot => Ok(builder.ins().bnot(val)),
                }
            }

            Node::Block {
                statements, result, ..
            } => {
                for stmt in statements {
                    self.lower_node(stmt, builder, var_map)?;
                }
                self.lower_node(result, builder, var_map)
            }

            other => Err(CompileError::UnsupportedNode(format!("{other}"))),
        }
    }

    fn lower_call(
        &mut self,
        target: &str,
        args: &[Node],
        node_type: &Type,
        builder: &mut FunctionBuilder,
        var_map: &mut HashMap<String, Variable>,
    ) -> Result<cranelift_codegen::ir::Value, CompileError> {
        match target {
            // --- I/O builtins ---
            "std::io::println" => {
                self.lower_println(args, builder, var_map)
            }
            "std::io::print" => {
                self.lower_print_no_newline(args, builder, var_map)
            }

            // --- String builtins ---
            "std::string::concat" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let b = self.lower_node(&args[1], builder, var_map)?;
                let id = self.str_concat_id;
                Ok(self.call_runtime(id, &[a, b], builder, true))
            }
            "std::string::len" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let id = self.str_len_id;
                Ok(self.call_runtime(id, &[a], builder, true))
            }
            "std::string::from_i64" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let id = self.str_from_i64_id;
                Ok(self.call_runtime(id, &[a], builder, true))
            }
            "std::string::contains" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let b = self.lower_node(&args[1], builder, var_map)?;
                let id = self.str_contains_id;
                Ok(self.call_runtime(id, &[a, b], builder, true))
            }

            // --- Math builtins ---
            "std::math::abs" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let id = self.math_abs_id;
                Ok(self.call_runtime(id, &[a], builder, true))
            }
            "std::math::max" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let b = self.lower_node(&args[1], builder, var_map)?;
                let id = self.math_max_id;
                Ok(self.call_runtime(id, &[a, b], builder, true))
            }
            "std::math::min" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let b = self.lower_node(&args[1], builder, var_map)?;
                let id = self.math_min_id;
                Ok(self.call_runtime(id, &[a, b], builder, true))
            }
            "std::math::pow" => {
                let a = self.lower_node(&args[0], builder, var_map)?;
                let b = self.lower_node(&args[1], builder, var_map)?;
                let id = self.math_pow_id;
                Ok(self.call_runtime(id, &[a, b], builder, true))
            }

            // --- User-defined function call ---
            _ => {
                let callee_id = self.func_ids.get(target).ok_or_else(|| {
                    CompileError::FunctionNotFound(target.to_string())
                })?;
                let callee_id = *callee_id;
                let func_ref = self
                    .jit_module
                    .declare_func_in_func(callee_id, builder.func);
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.lower_node(arg, builder, var_map)?);
                }
                let call = builder.ins().call(func_ref, &arg_vals);
                if matches!(node_type, Type::Unit) {
                    Ok(builder.ins().iconst(cl_types::I64, 0))
                } else {
                    Ok(builder.inst_results(call)[0])
                }
            }
        }
    }

    fn lower_println(
        &mut self,
        args: &[Node],
        builder: &mut FunctionBuilder,
        var_map: &mut HashMap<String, Variable>,
    ) -> Result<cranelift_codegen::ir::Value, CompileError> {
        if let Some(arg) = args.first() {
            // Determine if the argument is a string type
            if is_string_typed(arg) {
                // Use handle-based println
                let val = self.lower_node(arg, builder, var_map)?;
                let id = self.println_handle_id;
                self.call_runtime(id, &[val], builder, false);
            } else if let Node::Literal {
                value: LiteralValue::Str(s),
                ..
            } = arg
            {
                // Static string literal - use ptr+len
                let ptr_type = self.jit_module.target_config().pointer_type();
                let ptr_val = builder.ins().iconst(ptr_type, s.as_ptr() as i64);
                let len_val = builder.ins().iconst(cl_types::I64, s.len() as i64);
                let id = self.print_str_id;
                let func_ref = self.jit_module.declare_func_in_func(id, builder.func);
                builder.ins().call(func_ref, &[ptr_val, len_val]);
            } else {
                // Numeric - print as i64
                let val = self.lower_node(arg, builder, var_map)?;
                let id = self.print_i64_id;
                self.call_runtime(id, &[val], builder, false);
            }
        }
        Ok(builder.ins().iconst(cl_types::I64, 0))
    }

    fn lower_print_no_newline(
        &mut self,
        args: &[Node],
        builder: &mut FunctionBuilder,
        var_map: &mut HashMap<String, Variable>,
    ) -> Result<cranelift_codegen::ir::Value, CompileError> {
        if let Some(arg) = args.first() {
            if is_string_typed(arg) {
                let val = self.lower_node(arg, builder, var_map)?;
                let id = self.print_handle_id;
                self.call_runtime(id, &[val], builder, false);
            } else {
                let val = self.lower_node(arg, builder, var_map)?;
                let id = self.print_i64_id;
                self.call_runtime(id, &[val], builder, false);
            }
        }
        Ok(builder.ins().iconst(cl_types::I64, 0))
    }

    fn lower_binop(
        &self,
        op: &BinOpKind,
        lhs: cranelift_codegen::ir::Value,
        rhs: cranelift_codegen::ir::Value,
        builder: &mut FunctionBuilder,
    ) -> Result<cranelift_codegen::ir::Value, CompileError> {
        match op {
            BinOpKind::Add => Ok(builder.ins().iadd(lhs, rhs)),
            BinOpKind::Sub => Ok(builder.ins().isub(lhs, rhs)),
            BinOpKind::Mul => Ok(builder.ins().imul(lhs, rhs)),
            BinOpKind::Div => Ok(builder.ins().sdiv(lhs, rhs)),
            BinOpKind::Mod => Ok(builder.ins().srem(lhs, rhs)),
            BinOpKind::Eq => {
                let cmp = builder.ins().icmp(IntCC::Equal, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::Neq => {
                let cmp = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::Lt => {
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::Lte => {
                let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::Gt => {
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::Gte => {
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                Ok(builder.ins().uextend(cl_types::I64, cmp))
            }
            BinOpKind::And => Ok(builder.ins().band(lhs, rhs)),
            BinOpKind::Or => Ok(builder.ins().bor(lhs, rhs)),
            BinOpKind::BitAnd => Ok(builder.ins().band(lhs, rhs)),
            BinOpKind::BitOr => Ok(builder.ins().bor(lhs, rhs)),
            BinOpKind::BitXor => Ok(builder.ins().bxor(lhs, rhs)),
            BinOpKind::Shl => Ok(builder.ins().ishl(lhs, rhs)),
            BinOpKind::Shr => Ok(builder.ins().sshr(lhs, rhs)),
        }
    }
}

/// Check if a node produces a string-typed value (used to decide handle vs i64 printing).
fn is_string_typed(node: &Node) -> bool {
    match node {
        Node::Literal {
            value: LiteralValue::Str(_),
            ..
        } => true,
        Node::Call { target, .. } => matches!(
            target.as_str(),
            "std::string::concat"
                | "std::string::from_i64"
                | "std::fmt::format"
                | "std::io::read_file"
                | "std::io::read_line"
        ),
        Node::Param { node_type, .. } | Node::Let { node_type, .. } => {
            matches!(node_type, Type::String)
        }
        Node::If { node_type, .. } => matches!(node_type, Type::String),
        _ => false,
    }
}
