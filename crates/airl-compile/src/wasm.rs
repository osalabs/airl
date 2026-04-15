//! AIRL IR → WASM compilation.
//!
//! Translates AIRL IR directly to WASM bytecode using the `wasm-encoder` crate.
//! The generated module uses WASI for I/O (`fd_write` for stdout).
//!
//! ## Design
//! - Integer values are `i64` in WASM
//! - String literals are stored in a data segment; at runtime, strings are (offset, len) pairs
//! - `println` calls `fd_write` on stdout (fd=1)
//! - User-defined functions map 1:1 to WASM functions

use std::collections::HashMap;

use airl_ir::module::{FuncDef, Module};
use airl_ir::node::{BinOpKind, LiteralValue, Node, UnaryOpKind};
use airl_ir::types::Type;
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, ImportSection, Instruction, MemorySection, MemoryType, TypeSection, ValType,
};

use crate::CompileError;

/// Compile an AIRL module to a WASM binary (bytes).
pub fn compile_to_wasm(module: &Module) -> Result<Vec<u8>, CompileError> {
    module
        .find_function("main")
        .ok_or(CompileError::NoMainFunction)?;

    let mut compiler = WasmCompiler::new(module);
    compiler.compile()?;
    Ok(compiler.finish())
}

// ---------------------------------------------------------------------------
// WASM Compiler
// ---------------------------------------------------------------------------

struct WasmCompiler<'a> {
    module: &'a Module,
    /// String data segment: all string literals concatenated
    string_data: Vec<u8>,
    /// Map from string literal content to (offset, length) in data segment
    string_offsets: HashMap<String, (u32, u32)>,
    /// Map from AIRL function name to WASM function index
    func_indices: HashMap<String, u32>,
    /// Number of imported functions (offset for local function indices)
    import_count: u32,
}

impl<'a> WasmCompiler<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            string_data: Vec::new(),
            string_offsets: HashMap::new(),
            func_indices: HashMap::new(),
            import_count: 0,
        }
    }

    fn collect_strings(&mut self) {
        for func in self.module.functions() {
            self.collect_strings_from_node(&func.body);
        }
    }

    fn collect_strings_from_node(&mut self, node: &Node) {
        match node {
            Node::Literal {
                value: LiteralValue::Str(s),
                ..
            } => {
                if !self.string_offsets.contains_key(s) {
                    let offset = self.string_data.len() as u32;
                    let bytes = s.as_bytes();
                    self.string_data.extend_from_slice(bytes);
                    self.string_offsets
                        .insert(s.clone(), (offset, bytes.len() as u32));
                }
            }
            Node::Let { value, body, .. } => {
                self.collect_strings_from_node(value);
                self.collect_strings_from_node(body);
            }
            Node::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_strings_from_node(cond);
                self.collect_strings_from_node(then_branch);
                self.collect_strings_from_node(else_branch);
            }
            Node::Call { args, .. } => {
                for arg in args {
                    self.collect_strings_from_node(arg);
                }
            }
            Node::Return { value, .. } => self.collect_strings_from_node(value),
            Node::BinOp { lhs, rhs, .. } => {
                self.collect_strings_from_node(lhs);
                self.collect_strings_from_node(rhs);
            }
            Node::UnaryOp { operand, .. } => self.collect_strings_from_node(operand),
            Node::Block {
                statements, result, ..
            } => {
                for s in statements {
                    self.collect_strings_from_node(s);
                }
                self.collect_strings_from_node(result);
            }
            Node::Match {
                scrutinee, arms, ..
            } => {
                self.collect_strings_from_node(scrutinee);
                for arm in arms {
                    self.collect_strings_from_node(&arm.body);
                }
            }
            _ => {}
        }
    }

    fn compile(&mut self) -> Result<(), CompileError> {
        // Collect string literals for the data segment
        // Also add a newline character
        self.collect_strings();
        let newline_offset = self.string_data.len() as u32;
        self.string_data.push(b'\n');
        self.string_offsets
            .insert("\n".to_string(), (newline_offset, 1));

        // Assign function indices
        // Import: fd_write is index 0
        // Internal: __print_i64 is index 1
        self.import_count = 1;
        let internal_helper_count = 1u32; // __print_i64

        // User functions start after imports + internal helpers
        let user_func_base = self.import_count + internal_helper_count;
        for (i, func) in self.module.functions().iter().enumerate() {
            self.func_indices
                .insert(func.name.clone(), user_func_base + i as u32);
        }

        Ok(())
    }

    /// Build the internal `__print_i64` WASM function.
    /// Memory layout for scratch: offset 16..48 (32 bytes for digits).
    /// Algorithm: convert i64 to decimal string right-to-left, then fd_write.
    fn build_print_i64_func(&self) -> Function {
        // Locals: val (param 0), is_neg, digit_count, scratch_pos, digit, quotient
        // We need 5 extra locals (i64) beyond the parameter
        let mut f = Function::new(vec![(5, ValType::I64), (2, ValType::I32)]);

        // Local indices:
        // 0: val (param, i64)
        // 1: is_neg (i64)
        // 2: digit_count (i64)
        // 3: scratch_end = 48 (i64, constant)
        // 4: current_pos (i64)
        // 5: temp (i64)
        // 6: fd_write result (i32)
        // 7: temp i32

        let val = 0u32;
        let is_neg = 1u32;
        let digit_count = 2u32;
        let current_pos = 4u32;
        let temp64 = 5u32;

        // Handle zero case
        f.instruction(&Instruction::LocalGet(val));
        f.instruction(&Instruction::I64Eqz);
        f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        {
            // Store '0' at offset 47
            f.instruction(&Instruction::I32Const(47));
            f.instruction(&Instruction::I32Const(48)); // '0' = 48
            f.instruction(&Instruction::I32Store8(wasm_encoder::MemArg {
                offset: 0,
                align: 0,
                memory_index: 0,
            }));
            // Write iov: ptr=47, len=1
            f.instruction(&Instruction::I32Const(0));
            f.instruction(&Instruction::I32Const(47));
            f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
            f.instruction(&Instruction::I32Const(4));
            f.instruction(&Instruction::I32Const(1));
            f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
            f.instruction(&Instruction::I32Const(1)); // fd=stdout
            f.instruction(&Instruction::I32Const(0)); // iovs
            f.instruction(&Instruction::I32Const(1)); // iovs_len
            f.instruction(&Instruction::I32Const(8)); // nwritten
            f.instruction(&Instruction::Call(0)); // fd_write
            f.instruction(&Instruction::Drop);
            f.instruction(&Instruction::Return);
        }
        f.instruction(&Instruction::End);

        // Check if negative
        f.instruction(&Instruction::I64Const(0));
        f.instruction(&Instruction::LocalSet(is_neg));
        f.instruction(&Instruction::LocalGet(val));
        f.instruction(&Instruction::I64Const(0));
        f.instruction(&Instruction::I64LtS);
        f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        {
            f.instruction(&Instruction::I64Const(1));
            f.instruction(&Instruction::LocalSet(is_neg));
            // val = -val
            f.instruction(&Instruction::I64Const(0));
            f.instruction(&Instruction::LocalGet(val));
            f.instruction(&Instruction::I64Sub);
            f.instruction(&Instruction::LocalSet(val));
        }
        f.instruction(&Instruction::End);

        // current_pos = 47 (write digits right-to-left into memory[16..48])
        f.instruction(&Instruction::I64Const(47));
        f.instruction(&Instruction::LocalSet(current_pos));

        // digit_count = 0
        f.instruction(&Instruction::I64Const(0));
        f.instruction(&Instruction::LocalSet(digit_count));

        // Loop: extract digits
        f.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
        f.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
        {
            // digit = val % 10
            f.instruction(&Instruction::LocalGet(val));
            f.instruction(&Instruction::I64Const(10));
            f.instruction(&Instruction::I64RemU);
            f.instruction(&Instruction::LocalSet(temp64));

            // memory[current_pos] = digit + '0'
            f.instruction(&Instruction::LocalGet(current_pos));
            f.instruction(&Instruction::I32WrapI64);
            f.instruction(&Instruction::LocalGet(temp64));
            f.instruction(&Instruction::I64Const(48)); // '0'
            f.instruction(&Instruction::I64Add);
            f.instruction(&Instruction::I32WrapI64);
            f.instruction(&Instruction::I32Store8(wasm_encoder::MemArg {
                offset: 0,
                align: 0,
                memory_index: 0,
            }));

            // current_pos--
            f.instruction(&Instruction::LocalGet(current_pos));
            f.instruction(&Instruction::I64Const(1));
            f.instruction(&Instruction::I64Sub);
            f.instruction(&Instruction::LocalSet(current_pos));

            // digit_count++
            f.instruction(&Instruction::LocalGet(digit_count));
            f.instruction(&Instruction::I64Const(1));
            f.instruction(&Instruction::I64Add);
            f.instruction(&Instruction::LocalSet(digit_count));

            // val = val / 10
            f.instruction(&Instruction::LocalGet(val));
            f.instruction(&Instruction::I64Const(10));
            f.instruction(&Instruction::I64DivU);
            f.instruction(&Instruction::LocalSet(val));

            // if val > 0, continue loop
            f.instruction(&Instruction::LocalGet(val));
            f.instruction(&Instruction::I64Const(0));
            f.instruction(&Instruction::I64GtU);
            f.instruction(&Instruction::BrIf(0)); // branch to loop
        }
        f.instruction(&Instruction::End); // end loop
        f.instruction(&Instruction::End); // end block

        // If negative, prepend '-'
        f.instruction(&Instruction::LocalGet(is_neg));
        f.instruction(&Instruction::I64Const(1));
        f.instruction(&Instruction::I64Eq);
        f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        {
            f.instruction(&Instruction::LocalGet(current_pos));
            f.instruction(&Instruction::I32WrapI64);
            f.instruction(&Instruction::I32Const(45)); // '-'
            f.instruction(&Instruction::I32Store8(wasm_encoder::MemArg {
                offset: 0,
                align: 0,
                memory_index: 0,
            }));
            f.instruction(&Instruction::LocalGet(current_pos));
            f.instruction(&Instruction::I64Const(1));
            f.instruction(&Instruction::I64Sub);
            f.instruction(&Instruction::LocalSet(current_pos));
            f.instruction(&Instruction::LocalGet(digit_count));
            f.instruction(&Instruction::I64Const(1));
            f.instruction(&Instruction::I64Add);
            f.instruction(&Instruction::LocalSet(digit_count));
        }
        f.instruction(&Instruction::End);

        // Set up iov: ptr = current_pos + 1, len = digit_count
        f.instruction(&Instruction::I32Const(0)); // iov[0].buf addr
        f.instruction(&Instruction::LocalGet(current_pos));
        f.instruction(&Instruction::I64Const(1));
        f.instruction(&Instruction::I64Add);
        f.instruction(&Instruction::I32WrapI64);
        f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));

        f.instruction(&Instruction::I32Const(4)); // iov[0].len addr
        f.instruction(&Instruction::LocalGet(digit_count));
        f.instruction(&Instruction::I32WrapI64);
        f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));

        // fd_write(1, 0, 1, 8)
        f.instruction(&Instruction::I32Const(1));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(1));
        f.instruction(&Instruction::I32Const(8));
        f.instruction(&Instruction::Call(0));
        f.instruction(&Instruction::Drop);

        f.instruction(&Instruction::End);
        f
    }

    fn finish(&self) -> Vec<u8> {
        let mut wasm_module = wasm_encoder::Module::new();

        // --- Type section ---
        // We need type signatures for: fd_write, and each user function
        let mut types = TypeSection::new();

        // Type 0: fd_write(fd: i32, iovs: i32, iovs_len: i32, nwritten: i32) -> i32
        types.ty().function(
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I32],
        );

        // Type 1: __print_i64(val: i64) -> void
        types.ty().function(vec![ValType::I64], vec![]);

        // Type indices for user functions start at 2
        let mut func_type_map: HashMap<String, u32> = HashMap::new();
        for (i, func) in self.module.functions().iter().enumerate() {
            let type_idx = (i + 2) as u32;
            let params: Vec<ValType> = func.params.iter().map(|_| ValType::I64).collect();
            let results: Vec<ValType> = if matches!(func.returns, Type::Unit) {
                vec![]
            } else {
                vec![ValType::I64]
            };
            types.ty().function(params, results);
            func_type_map.insert(func.name.clone(), type_idx);
        }

        wasm_module.section(&types);

        // --- Import section ---
        let mut imports = ImportSection::new();
        imports.import(
            "wasi_snapshot_preview1",
            "fd_write",
            EntityType::Function(0),
        );
        wasm_module.section(&imports);

        // --- Function section ---
        let mut functions = FunctionSection::new();
        // Internal helper: __print_i64 (type 1)
        functions.function(1);
        // User functions
        for func in self.module.functions() {
            let type_idx = func_type_map[&func.name];
            functions.function(type_idx);
        }
        wasm_module.section(&functions);

        // --- Memory section ---
        let mut memories = MemorySection::new();
        // 1 page = 64KB, enough for string data + iov buffer
        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        wasm_module.section(&memories);

        // --- Export section ---
        let mut exports = ExportSection::new();
        // Export memory for WASI
        exports.export("memory", ExportKind::Memory, 0);
        // Export _start (main function)
        if let Some(&main_idx) = self.func_indices.get("main") {
            exports.export("_start", ExportKind::Func, main_idx);
        }
        wasm_module.section(&exports);

        // --- Code section ---
        let mut codes = CodeSection::new();
        // Internal helper: __print_i64
        let print_i64_func = self.build_print_i64_func();
        codes.function(&print_i64_func);
        // User functions
        for func in self.module.functions() {
            let wasm_func = self.compile_function(func);
            codes.function(&wasm_func);
        }
        wasm_module.section(&codes);

        // --- Data section ---
        // Store string literals in linear memory starting at offset 1024
        // Reserve 0-1023 for the iov buffer area
        let mut data = DataSection::new();
        if !self.string_data.is_empty() {
            data.active(
                0,
                &ConstExpr::i32_const(1024),
                self.string_data.iter().copied(),
            );
        }
        wasm_module.section(&data);

        wasm_module.finish()
    }

    fn compile_function(&self, func: &FuncDef) -> Function {
        let mut local_count = func.params.len() as u32;
        let mut local_names: HashMap<String, u32> = HashMap::new();

        // Parameters are locals 0..n-1
        for (i, param) in func.params.iter().enumerate() {
            local_names.insert(param.name.clone(), i as u32);
        }

        // We'll accumulate extra locals as let-bindings
        let mut extra_locals: u32 = 0;
        count_let_bindings(&func.body, &mut extra_locals);

        let mut f = Function::new(vec![(extra_locals, ValType::I64)]);

        self.emit_node(&func.body, &mut f, &mut local_names, &mut local_count, func);

        // If the function returns Unit, don't leave anything on the stack
        // (emit_node for blocks may leave a value; we drop it)
        if matches!(func.returns, Type::Unit) {
            // The body should have handled cleanup, but ensure nothing extra on stack
        }

        f.instruction(&Instruction::End);
        f
    }

    fn emit_node(
        &self,
        node: &Node,
        f: &mut Function,
        locals: &mut HashMap<String, u32>,
        local_count: &mut u32,
        func_def: &FuncDef,
    ) {
        match node {
            Node::Literal { value, .. } => match value {
                LiteralValue::Integer(i) => {
                    f.instruction(&Instruction::I64Const(*i));
                }
                LiteralValue::Boolean(b) => {
                    f.instruction(&Instruction::I64Const(if *b { 1 } else { 0 }));
                }
                LiteralValue::Unit => {
                    // For unit in non-void contexts, push 0
                    f.instruction(&Instruction::I64Const(0));
                }
                LiteralValue::Float(_f_val) => {
                    // WASM f64
                    f.instruction(&Instruction::I64Const(0)); // simplified
                }
                LiteralValue::Str(_s) => {
                    // Push string offset as i64 (for identification purposes)
                    if let Some(&(offset, _len)) = self.string_offsets.get(_s) {
                        f.instruction(&Instruction::I64Const((1024 + offset) as i64));
                    } else {
                        f.instruction(&Instruction::I64Const(0));
                    }
                }
            },

            Node::Param { name, .. } => {
                if let Some(&idx) = locals.get(name) {
                    f.instruction(&Instruction::LocalGet(idx));
                } else {
                    f.instruction(&Instruction::I64Const(0));
                }
            }

            Node::Let {
                name, value, body, ..
            } => {
                // Allocate a new local for this binding
                let idx = *local_count;
                *local_count += 1;
                locals.insert(name.clone(), idx);

                self.emit_node(value, f, locals, local_count, func_def);
                f.instruction(&Instruction::LocalSet(idx));
                self.emit_node(body, f, locals, local_count, func_def);

                locals.remove(name);
            }

            Node::If {
                cond,
                then_branch,
                else_branch,
                node_type,
                ..
            } => {
                self.emit_node(cond, f, locals, local_count, func_def);
                // Convert i64 to i32 for br_if
                f.instruction(&Instruction::I32WrapI64);

                let block_type = if matches!(node_type, Type::Unit) {
                    wasm_encoder::BlockType::Empty
                } else {
                    wasm_encoder::BlockType::Result(ValType::I64)
                };

                f.instruction(&Instruction::If(block_type));
                self.emit_node(then_branch, f, locals, local_count, func_def);
                f.instruction(&Instruction::Else);
                self.emit_node(else_branch, f, locals, local_count, func_def);
                f.instruction(&Instruction::End);
            }

            Node::Call { target, args, .. } => {
                if target == "std::io::println" {
                    self.emit_println(args, f, locals, local_count, func_def);
                } else if let Some(&func_idx) = self.func_indices.get(target.as_str()) {
                    // Emit arguments
                    for arg in args {
                        self.emit_node(arg, f, locals, local_count, func_def);
                    }
                    f.instruction(&Instruction::Call(func_idx));
                } else {
                    // Unknown function - push 0
                    f.instruction(&Instruction::I64Const(0));
                }
            }

            Node::Return { value, .. } => {
                self.emit_node(value, f, locals, local_count, func_def);
                f.instruction(&Instruction::Return);
            }

            Node::BinOp { op, lhs, rhs, .. } => {
                self.emit_node(lhs, f, locals, local_count, func_def);
                self.emit_node(rhs, f, locals, local_count, func_def);
                self.emit_binop(op, f);
            }

            Node::UnaryOp { op, operand, .. } => {
                self.emit_node(operand, f, locals, local_count, func_def);
                match op {
                    UnaryOpKind::Neg => {
                        // 0 - value
                        f.instruction(&Instruction::I64Const(0));
                        // Swap: we need 0 on bottom, value on top
                        // Actually re-emit: push 0 first, then value, then sub
                        // Let's re-do: we already have value on stack. We need (0 - value)
                        // Use a local to swap
                        let tmp = *local_count;
                        *local_count += 1;
                        f.instruction(&Instruction::LocalSet(tmp));
                        f.instruction(&Instruction::I64Const(0));
                        f.instruction(&Instruction::LocalGet(tmp));
                        f.instruction(&Instruction::I64Sub);
                    }
                    UnaryOpKind::Not => {
                        f.instruction(&Instruction::I64Eqz);
                        f.instruction(&Instruction::I64ExtendI32U);
                    }
                    UnaryOpKind::BitNot => {
                        f.instruction(&Instruction::I64Const(-1));
                        f.instruction(&Instruction::I64Xor);
                    }
                }
            }

            Node::Block {
                statements, result, ..
            } => {
                for stmt in statements {
                    self.emit_node(stmt, f, locals, local_count, func_def);
                    // If statement leaves a value on the stack and it's not the result,
                    // we need to drop it. For Call nodes that return Unit, nothing is left.
                    // For safety, we check if the statement is a Call returning Unit
                    if leaves_value_on_stack(stmt) {
                        f.instruction(&Instruction::Drop);
                    }
                }
                self.emit_node(result, f, locals, local_count, func_def);
            }

            Node::Loop {
                body, node_type, ..
            } => {
                let block_type = if matches!(node_type, Type::Unit) {
                    wasm_encoder::BlockType::Empty
                } else {
                    wasm_encoder::BlockType::Result(ValType::I64)
                };
                // WASM loop: block { loop { body; br loop; } }
                f.instruction(&Instruction::Block(block_type));
                f.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
                self.emit_node(body, f, locals, local_count, func_def);
                // Branch back to the loop header (index 0 = inner loop)
                f.instruction(&Instruction::Br(0));
                f.instruction(&Instruction::End); // end loop
                f.instruction(&Instruction::End); // end block
                                                  // If non-unit type needed, push default
                if !matches!(node_type, Type::Unit) {
                    f.instruction(&Instruction::I64Const(0));
                }
            }

            Node::Match {
                scrutinee,
                arms,
                node_type,
                ..
            } => {
                use airl_ir::node::Pattern;

                // Evaluate scrutinee into a local
                self.emit_node(scrutinee, f, locals, local_count, func_def);
                let scrut_local = *local_count;
                *local_count += 1;
                f.instruction(&Instruction::LocalSet(scrut_local));

                // Result local (for non-unit matches)
                let result_local = *local_count;
                *local_count += 1;
                f.instruction(&Instruction::I64Const(0));
                f.instruction(&Instruction::LocalSet(result_local));

                // Use a block with nested if/else for each arm
                // Strategy: check each literal arm; if matched, set result and br out
                let num_literal_arms = arms
                    .iter()
                    .filter(|a| matches!(a.pattern, Pattern::Literal { .. }))
                    .count();
                let _ = num_literal_arms;

                // Outer block to break out of
                f.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));

                for arm in arms.iter() {
                    match &arm.pattern {
                        Pattern::Literal { value } => {
                            f.instruction(&Instruction::LocalGet(scrut_local));
                            match value {
                                LiteralValue::Integer(n) => {
                                    f.instruction(&Instruction::I64Const(*n));
                                }
                                LiteralValue::Boolean(b) => {
                                    f.instruction(&Instruction::I64Const(if *b { 1 } else { 0 }));
                                }
                                _ => {
                                    f.instruction(&Instruction::I64Const(0));
                                }
                            }
                            f.instruction(&Instruction::I64Eq);
                            f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                            self.emit_node(&arm.body, f, locals, local_count, func_def);
                            f.instruction(&Instruction::LocalSet(result_local));
                            f.instruction(&Instruction::Br(1)); // break out of outer block
                            f.instruction(&Instruction::End); // end if
                        }
                        Pattern::Wildcard | Pattern::Variable { .. } => {
                            if let Pattern::Variable { name } = &arm.pattern {
                                locals.insert(name.clone(), scrut_local);
                            }
                            self.emit_node(&arm.body, f, locals, local_count, func_def);
                            f.instruction(&Instruction::LocalSet(result_local));
                            // No need to br — this is the default
                        }
                    }
                }

                f.instruction(&Instruction::End); // end outer block

                // Push result
                if !matches!(node_type, Type::Unit) {
                    f.instruction(&Instruction::LocalGet(result_local));
                }
            }

            _ => {
                // Unsupported node - push 0
                f.instruction(&Instruction::I64Const(0));
            }
        }
    }

    fn emit_println(
        &self,
        args: &[Node],
        f: &mut Function,
        locals: &mut HashMap<String, u32>,
        local_count: &mut u32,
        func_def: &FuncDef,
    ) {
        if let Some(arg) = args.first() {
            match arg {
                Node::Literal {
                    value: LiteralValue::Str(s),
                    ..
                } => {
                    if let Some(&(offset, len)) = self.string_offsets.get(s) {
                        let mem_offset = 1024 + offset;
                        self.emit_fd_write_static(f, mem_offset, len);
                    }
                }
                _ => {
                    // Emit the argument, then call __print_i64 (func index 1)
                    self.emit_node(arg, f, locals, local_count, func_def);
                    f.instruction(&Instruction::Call(1)); // __print_i64
                }
            }
        }
        // Print newline
        if let Some(&(nl_offset, nl_len)) = self.string_offsets.get("\n") {
            let mem_offset = 1024 + nl_offset;
            self.emit_fd_write_static(f, mem_offset, nl_len);
        }
    }

    /// Emit fd_write for a static string at known memory offset.
    /// Uses memory addresses 0-15 as scratch for the iov struct.
    fn emit_fd_write_static(&self, f: &mut Function, offset: u32, len: u32) {
        // Write iov to memory at address 0:
        //   iov[0].buf = offset (i32 at addr 0)
        //   iov[0].len = len   (i32 at addr 4)
        f.instruction(&Instruction::I32Const(0)); // addr for buf ptr
        f.instruction(&Instruction::I32Const(offset as i32)); // value
        f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));

        f.instruction(&Instruction::I32Const(4)); // addr for buf len
        f.instruction(&Instruction::I32Const(len as i32)); // value
        f.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));

        // Call fd_write(fd=1, iovs=0, iovs_len=1, nwritten=8)
        f.instruction(&Instruction::I32Const(1)); // fd: stdout
        f.instruction(&Instruction::I32Const(0)); // iovs pointer
        f.instruction(&Instruction::I32Const(1)); // iovs count
        f.instruction(&Instruction::I32Const(8)); // nwritten pointer
        f.instruction(&Instruction::Call(0)); // fd_write (import index 0)
        f.instruction(&Instruction::Drop); // drop return value
    }

    fn emit_binop(&self, op: &BinOpKind, f: &mut Function) {
        match op {
            BinOpKind::Add => {
                f.instruction(&Instruction::I64Add);
            }
            BinOpKind::Sub => {
                f.instruction(&Instruction::I64Sub);
            }
            BinOpKind::Mul => {
                f.instruction(&Instruction::I64Mul);
            }
            BinOpKind::Div => {
                f.instruction(&Instruction::I64DivS);
            }
            BinOpKind::Mod => {
                f.instruction(&Instruction::I64RemS);
            }
            BinOpKind::Eq => {
                f.instruction(&Instruction::I64Eq);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::Neq => {
                f.instruction(&Instruction::I64Ne);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::Lt => {
                f.instruction(&Instruction::I64LtS);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::Lte => {
                f.instruction(&Instruction::I64LeS);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::Gt => {
                f.instruction(&Instruction::I64GtS);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::Gte => {
                f.instruction(&Instruction::I64GeS);
                f.instruction(&Instruction::I64ExtendI32U);
            }
            BinOpKind::And | BinOpKind::BitAnd => {
                f.instruction(&Instruction::I64And);
            }
            BinOpKind::Or | BinOpKind::BitOr => {
                f.instruction(&Instruction::I64Or);
            }
            BinOpKind::BitXor => {
                f.instruction(&Instruction::I64Xor);
            }
            BinOpKind::Shl => {
                f.instruction(&Instruction::I64Shl);
            }
            BinOpKind::Shr => {
                f.instruction(&Instruction::I64ShrS);
            }
        }
    }
}

/// Count how many let-bindings exist in a node tree (for pre-allocating locals).
fn count_let_bindings(node: &Node, count: &mut u32) {
    match node {
        Node::Let { value, body, .. } => {
            *count += 1;
            count_let_bindings(value, count);
            count_let_bindings(body, count);
        }
        Node::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            count_let_bindings(cond, count);
            count_let_bindings(then_branch, count);
            count_let_bindings(else_branch, count);
        }
        Node::Call { args, .. } => {
            for arg in args {
                count_let_bindings(arg, count);
            }
        }
        Node::Return { value, .. } => count_let_bindings(value, count),
        Node::BinOp { lhs, rhs, .. } => {
            count_let_bindings(lhs, count);
            count_let_bindings(rhs, count);
        }
        Node::UnaryOp { operand, .. } => count_let_bindings(operand, count),
        Node::Block {
            statements, result, ..
        } => {
            for s in statements {
                count_let_bindings(s, count);
            }
            count_let_bindings(result, count);
        }
        Node::Match {
            scrutinee, arms, ..
        } => {
            // Match needs 2 extra locals: scrutinee + result
            *count += 2;
            count_let_bindings(scrutinee, count);
            for arm in arms {
                count_let_bindings(&arm.body, count);
            }
        }
        Node::Loop { body, .. } => {
            count_let_bindings(body, count);
        }
        _ => {}
    }
    // Also count extra locals needed for UnaryOp::Neg
    if matches!(
        node,
        Node::UnaryOp {
            op: UnaryOpKind::Neg,
            ..
        }
    ) {
        *count += 1;
    }
}

/// Check if a node leaves a value on the WASM stack after execution.
fn leaves_value_on_stack(node: &Node) -> bool {
    match node {
        Node::Call { target, .. } => {
            // println/print return nothing visible
            !matches!(
                target.as_str(),
                "std::io::println" | "std::io::print" | "std::io::eprintln"
            )
        }
        Node::Let { .. } => false, // let bindings use local.set
        _ => false,
    }
}
