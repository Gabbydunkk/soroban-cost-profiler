use crate::models::CallStackNode;
use std::fmt::Write;

/// Converts the aggregated call tree into standard profiling formats (e.g., collapsed stack).
pub struct OutputFormatter;

impl OutputFormatter {
    /// Formats the tree into a collapsed stack efficiently.
    pub fn to_collapsed_stack(root: &CallStackNode) -> String {
        let mut output = String::with_capacity(1024); // Pre-allocate to optimize memory allocations
        let mut current_path = String::new();
        Self::format_node(root, &mut current_path, &mut output);
        output
    }

    /// Recursively walks the tree, avoiding unnecessary clones by using mutable string references.
    fn format_node(node: &CallStackNode, current_path: &mut String, output: &mut String) {
        let original_len = current_path.len();

        if !current_path.is_empty() {
            current_path.push(';');
        }
        current_path.push_str(&node.frame.function_name);

        // Folded format: `<path> <cost>`
        let _ = writeln!(output, "{} {}", current_path, node.exclusive_cpu);

        for child in node.children.values() {
            Self::format_node(child, current_path, output);
        }

        // Backtrack efficiently by truncating to the original length
        current_path.truncate(original_len);
    }
}
