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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CallStackNode, SourceFrame};

    /// Build a leaf node carrying the given exclusive cost.
    fn leaf(name: &str, exclusive_cpu: u64) -> CallStackNode {
        node(name, exclusive_cpu, Vec::new())
    }

    /// Build a node with the given children, keyed by their function name.
    fn node(name: &str, exclusive_cpu: u64, children: Vec<CallStackNode>) -> CallStackNode {
        CallStackNode {
            frame: SourceFrame {
                function_name: name.to_string(),
                file_path: None,
                line_number: None,
            },
            exclusive_cpu,
            inclusive_cpu: exclusive_cpu
                + children
                    .iter()
                    .map(|child| child.inclusive_cpu)
                    .sum::<u64>(),
            exclusive_mem: 0,
            inclusive_mem: 0,
            children: children
                .into_iter()
                .map(|child| (child.frame.function_name.clone(), child))
                .collect(),
        }
    }

    /// Sorted output lines, because `children` is a `HashMap` and emission order is not stable.
    fn lines(output: &str) -> Vec<&str> {
        let mut lines: Vec<&str> = output.lines().collect();
        lines.sort_unstable();
        lines
    }

    #[test]
    fn root_without_children_emits_one_line() {
        let output = OutputFormatter::to_collapsed_stack(&leaf("main", 42));

        assert_eq!(lines(&output), vec!["main 42"]);
    }

    #[test]
    fn path_is_semicolon_delimited_per_depth() {
        let tree = node("a", 1, vec![node("b", 2, vec![leaf("c", 3)])]);

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert_eq!(lines(&output), vec!["a 1", "a;b 2", "a;b;c 3"]);
    }

    #[test]
    fn every_node_emits_exactly_one_line() {
        let tree = node(
            "main",
            0,
            vec![leaf("alpha", 10), leaf("beta", 20), leaf("gamma", 30)],
        );

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert_eq!(output.lines().count(), 4);
        assert_eq!(
            lines(&output),
            vec!["main 0", "main;alpha 10", "main;beta 20", "main;gamma 30"]
        );
    }

    #[test]
    fn sibling_subtrees_do_not_contaminate_each_other() {
        // Guards the `truncate(original_len)` backtracking: without rewinding the path
        // after the "b" subtree, "c" would be emitted as "a;b;c".
        let tree = node(
            "a",
            1,
            vec![
                node("b", 2, vec![leaf("d", 4)]),
                node("c", 3, vec![leaf("e", 5)]),
            ],
        );

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert_eq!(
            lines(&output),
            vec!["a 1", "a;b 2", "a;b;d 4", "a;c 3", "a;c;e 5"]
        );
    }

    #[test]
    fn exclusive_cost_is_reported_not_inclusive() {
        let tree = node("main", 5, vec![leaf("callee", 95)]);

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert!(output.contains("main 5\n"), "got: {output:?}");
        assert!(!output.contains("main 100"));
    }

    #[test]
    fn zero_cost_frames_are_still_emitted() {
        // Folded stacks are merged by the consuming tool, so a zero-cost frame must stay
        // in the output to keep its call path intact.
        let tree = node("main", 0, vec![leaf("callee", 0)]);

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert_eq!(lines(&output), vec!["main 0", "main;callee 0"]);
    }

    #[test]
    fn output_ends_with_a_single_trailing_newline() {
        let tree = node("main", 1, vec![leaf("callee", 2)]);

        let output = OutputFormatter::to_collapsed_stack(&tree);

        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }
}
