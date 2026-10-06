use std::collections::HashMap;

/// Represents the kind of WASM execution event intercepted by the tracer.
///
/// * `Call`: Indicates that execution has crossed a function boundary into a new frame.
/// * `Return`: Indicates that the current function frame has exited.
/// * `Step`: Indicates that a WASM instruction (or sequence of instructions) was executed.
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    Call,
    Return,
    Step,
}

/// A snapshot of execution state emitted by the `ExecutionTracer`.
///
/// This model captures the program counter and associated costs for a specific `EventType`.
/// `cpu_cost` and `mem_cost` represent the delta (accumulated cost) since the last event
/// was emitted.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    pub pc: usize, // WASM Program Counter (Instruction Pointer)
    pub event_type: EventType,
    pub cpu_cost: u64, // CPU cost consumed since last event
    pub mem_cost: u64, // Memory allocated since last event
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFrame {
    pub function_name: String,
    pub file_path: Option<String>,
    pub line_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallStackNode {
    pub frame: SourceFrame,
    pub exclusive_cpu: u64, // CPU cost of this function itself
    pub inclusive_cpu: u64, // CPU cost of this function + all its children
    pub exclusive_mem: u64, // Mem cost of this function itself
    pub inclusive_mem: u64, // Mem cost of this function + all its children
    pub children: HashMap<String, CallStackNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equality() {
        let event1 = TraceEvent {
            pc: 10,
            event_type: EventType::Call,
            cpu_cost: 100,
            mem_cost: 200,
        };
        let event2 = event1.clone();
        assert_eq!(event1, event2);
    }
}
