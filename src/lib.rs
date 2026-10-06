pub mod aggregator;
pub mod formatter;
pub mod models;
pub mod source_map;
pub mod tracer;

#[cfg(test)]
mod tests {
    use crate::formatter::OutputFormatter;
    use crate::models::{CallStackNode, SourceFrame};
    use crate::tracer::ExecutionTracer;
    use std::collections::HashMap;

    #[test]
    fn test_formatter_to_collapsed_stack() {
        let mut root = CallStackNode {
            frame: SourceFrame {
                function_name: "main".to_string(),
                file_path: None,
                line_number: None,
            },
            exclusive_cpu: 10,
            inclusive_cpu: 100,
            exclusive_mem: 0,
            inclusive_mem: 0,
            children: HashMap::new(),
        };

        let child = CallStackNode {
            frame: SourceFrame {
                function_name: "compute".to_string(),
                file_path: None,
                line_number: None,
            },
            exclusive_cpu: 90,
            inclusive_cpu: 90,
            exclusive_mem: 0,
            inclusive_mem: 0,
            children: HashMap::new(),
        };

        root.children.insert("compute".to_string(), child);

        let output = OutputFormatter::to_collapsed_stack(&root);

        assert!(output.contains("main 10"));
        assert!(output.contains("main;compute 90"));
    }

    #[test]
    fn test_tracer_initialization() {
        let mut tracer = ExecutionTracer::new();
        assert!(!tracer.is_initialized);
        let events = tracer.trace();
        assert!(tracer.is_initialized);
        assert!(events.is_empty()); // Currently returns empty vec
    }
}
