use std::fmt;

use crate::common::{data::Data, number::build_number, opcode::Opcode, span::Span};

use crate::core::ffi::FFIFunction;

/// Represents a variable visible in the current scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Captured {
    /// The index on the stack if the variable is local to the current scope.
    Local(usize),
    /// The index of the upvalue in the enclosing scope.
    Nonlocal(usize),
}

/// Represents a single interpretable chunk of bytecode,
/// think a function.
#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    // TODO: make this a list of variable names
    // So structs can be made, and state preserved in the repl.
    /// Number of variables declared in this scope.
    pub decls: usize,
    /// Each byte is an opcode or a number-stream.
    pub code: Vec<u8>,
    /// Each usize indexes the bytecode op that begins each line.
    pub spans: Vec<(usize, Span)>,
    /// Number-stream indexed, used to load constants.
    pub constants: Vec<Data>,
    /// List of positions of locals in the scope where this lambda is defined,
    /// indexes must be gauranteed to be data on the heap.
    pub captures: Vec<Captured>,
    /// List of FFI functions (i.e. Rust functions)
    /// that can be called from this function.
    pub ffi: Vec<FFIFunction>,
}

impl Lambda {
    /// Creates a new empty `Lambda` to be filled.
    pub fn empty() -> Lambda {
        Lambda {
            decls: 0,
            code: vec![],
            spans: vec![],
            constants: vec![],
            captures: vec![],
            ffi: vec![],
        }
    }

    /// Emits an opcode as a byte.
    pub fn emit(&mut self, op: Opcode) {
        self.code.push(op as u8)
    }

    /// Emits a series of bytes.
    pub fn emit_bytes(&mut self, bytes: &mut Vec<u8>) {
        self.code.append(bytes)
    }

    /// Emits a span, should be called before an opcode is emmited.
    /// This function ties opcodes to spans in source.
    /// See index_span as well.
    pub fn emit_span(&mut self, span: &Span) {
        self.spans.push((self.code.len(), span.clone()))
    }

    /// Removes the last emitted byte.
    pub fn demit(&mut self) {
        self.code.pop();
    }

    /// Given some data, this function adds it to the constants table,
    /// and returns the data's index.
    /// The constants table is push only, so constants are identified by their index.
    /// The resulting usize can be split up into a number byte stream,
    /// and be inserted into the bytecode.
    pub fn index_data(&mut self, data: Data) -> usize {
        match self.constants.iter().position(|d| d == &data) {
            Some(d) => d,
            None => {
                self.constants.push(data);
                self.constants.len() - 1
            }
        }
    }

    /// Look up the nearest span at or before the index of a specific bytecode op.
    pub fn index_span(&self, index: usize) -> Span {
        let mut best = &Span::empty();

        for (i, span) in self.spans.iter() {
            if i > &index {
                break;
            }
            best = span;
        }

        best.clone()
    }

    /// Adds a ffi function to the ffi table,
    /// without checking for duplicates.
    /// The `Compiler` ensures that functions are valid
    /// and not duplicated during codegen.
    pub fn add_ffi(&mut self, function: FFIFunction) -> usize {
        self.ffi.push(function);
        self.ffi.len() - 1
    }
}

impl fmt::Display for Lambda {
    /// Dump a human-readable breakdown of a `Lambda`'s bytecode.
    /// Including constants, captures, and variables declared.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "-- Dumping Constants:")?;
        for constant in self.constants.iter() {
            writeln!(f, "{:?}", constant)?;
        }

        // writeln!(f, "-- Dumping Spans:")?;
        // for span in self.spans.iter() {
        //     writeln!(f, "{:?}", span)?;
        // }

        writeln!(f, "-- Dumping Captures:")?;
        for capture in self.captures.iter() {
            writeln!(f, "{:?}", capture)?;
        }

        writeln!(f, "-- Dumping Variables: {}", self.decls)?;

        writeln!(f, "-- Dumping Bytecode:")?;
        writeln!(f, "Inst.   \tArgs\tValue?")?;
        let mut index = 0;

        while index < self.code.len() {
            index += 1;
            match Opcode::from_byte(self.code[index - 1]) {
                Opcode::Con => {
                    let (constant_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(
                        f,
                        "Load Con\t{}\t{:?}",
                        constant_index, self.constants[constant_index]
                    )?;
                }
                Opcode::NotInit => {
                    writeln!(f, "NotInit \t\tDeclare variable")?;
                }
                Opcode::Del => {
                    writeln!(f, "Delete  \t\t--")?;
                }
                Opcode::Capture => {
                    let (local_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Capture \t{}\tIndexed local moved to heap", local_index)?;
                }
                Opcode::Save => {
                    let (local_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Save    \t{}\tIndexed local", local_index)?;
                }
                Opcode::SaveCap => {
                    let (upvalue_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Save Cap\t{}\tIndexed upvalue on heap", upvalue_index)?;
                }
                Opcode::Load => {
                    let (local_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Load    \t{}\tIndexed local", local_index)?;
                }
                Opcode::LoadCap => {
                    let (upvalue_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Load Cap\t{}\tIndexed upvalue on heap", upvalue_index)?;
                }
                Opcode::Call => {
                    writeln!(f, "Call    \t\tRun top function using next stack value")?;
                }
                Opcode::Return => {
                    let (num_locals, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Return  \t{}\tLocals on stack deleted", num_locals)?;
                }
                Opcode::Closure => {
                    let (todo_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Closure \t{}\tIndex of lambda to be wrapped", todo_index)?;
                }
                Opcode::Print => {
                    writeln!(f, "Print    \t\t--")?;
                }
                Opcode::Label => {
                    writeln!(f, "Label    \t\t--")?;
                }
                Opcode::Tuple => {
                    let (length, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Tuple   \t{}\tValues tupled together", length)?;
                }
                Opcode::UnLabel => {
                    writeln!(f, "UnLabel  \t\t--")?;
                }
                Opcode::UnData => {
                    writeln!(f, "UnData   \t\t--")?;
                }
                Opcode::UnTuple => {
                    let (item_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "UnTuple \t{}\tItem accessed", item_index)?;
                }
                Opcode::Copy => {
                    writeln!(f, "Copy     \t\t--")?;
                }
                Opcode::FFICall => {
                    let (ffi_index, consumed) = build_number(&self.code[index..]);
                    index += consumed;
                    writeln!(f, "Return  \t{}\tIndexed FFI function called", ffi_index)?;
                }
            }
        }

        Ok(())
    }
}
