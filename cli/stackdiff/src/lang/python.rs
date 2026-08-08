//! Python callable extraction (tree-sitter-python).
//! Faithful port of calldiff's python.ts, including constructor aliasing,
//! underscore privacy, and module-level lambda assignments.

use tree_sitter::{Node, Tree};

use super::{child_by_type, collapse_ws, named_children, text};
use crate::types::{CallStep, FunctionInfo};

fn params_label(params: Option<Node>, src: &str) -> String {
    let Some(params) = params else {
        return "()".to_string();
    };
    if params.kind() != "parameters" && params.kind() != "lambda_parameters" {
        return "()".to_string();
    }
    let mut names: Vec<String> = Vec::new();
    for p in named_children(params) {
        match p.kind() {
            "identifier" => names.push(text(p, src).to_string()),
            "list_splat_pattern" | "dictionary_splat_pattern" => {
                let id = child_by_type(p, "identifier");
                names.push(match id {
                    Some(id) => format!("*{}", text(id, src)),
                    None => "*".to_string(),
                });
            }
            "default_parameter" => {
                let id = child_by_type(p, "identifier");
                names.push(
                    id.map(|id| text(id, src).to_string())
                        .unwrap_or_else(|| "_".to_string()),
                );
            }
            _ => names.push("_".to_string()),
        }
    }
    if names.is_empty() {
        "()".to_string()
    } else {
        format!("({})", names.join(", "))
    }
}

fn callee_key(node: Node, class_name: Option<&str>, src: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(text(node, src).to_string()),
        "attribute" => {
            let object = node.named_child(0)?;
            let attr = node.named_child(1)?;
            let prop = text(attr, src);
            if object.kind() == "identifier" {
                let obj = text(object, src);
                if let Some(class_name) = class_name.filter(|_| obj == "self" || obj == "cls") {
                    return Some(format!("{class_name}.{prop}"));
                }
                return Some(format!("{obj}.{prop}"));
            }
            if let Some(class_name) = class_name {
                return Some(format!("{class_name}.{prop}"));
            }
            Some(prop.to_string())
        }
        _ => None,
    }
}

fn collect_block(block: Option<Node>, class_name: Option<&str>, src: &str) -> Vec<CallStep> {
    match block {
        Some(block) => collect_statements(named_children(block), class_name, src),
        None => Vec::new(),
    }
}

struct Collector<'s> {
    src: &'s str,
    steps: Vec<CallStep>,
    seen: std::collections::HashSet<String>,
}

impl<'s> Collector<'s> {
    fn add_call(&mut self, key: String, start: usize) {
        let mark = format!("{key}:{start}");
        if self.seen.contains(&mark) {
            return;
        }
        self.seen.insert(mark);
        self.steps.push(CallStep::Call { key });
    }

    fn walk(&mut self, node: Node, class_name: Option<&str>) {
        match node.kind() {
            "function_definition" | "class_definition" | "lambda" => return,
            "if_statement" => {
                let cond_node = named_children(node).into_iter().find(|c| {
                    c.kind() != "block" && c.kind() != "else_clause" && c.kind() != "elif_clause"
                });
                let cond = cond_node
                    .map(|c| collapse_ws(text(c, self.src)))
                    .unwrap_or_default();
                let body = child_by_type(node, "block");
                let children = collect_block(body, class_name, self.src);
                self.steps.push(CallStep::Branch {
                    key: if cond.is_empty() {
                        "if".to_string()
                    } else {
                        format!("if:{cond}")
                    },
                    label: if cond.is_empty() {
                        "if".to_string()
                    } else {
                        format!("if {cond}")
                    },
                    children,
                });

                for clause in named_children(node) {
                    if clause.kind() == "elif_clause" {
                        let elif_cond = named_children(clause)
                            .into_iter()
                            .find(|c| c.kind() != "block");
                        let cond_text = elif_cond
                            .map(|c| collapse_ws(text(c, self.src)))
                            .unwrap_or_default();
                        let children =
                            collect_block(child_by_type(clause, "block"), class_name, self.src);
                        self.steps.push(CallStep::Branch {
                            key: if cond_text.is_empty() {
                                "else-if".to_string()
                            } else {
                                format!("else-if:{cond_text}")
                            },
                            label: if cond_text.is_empty() {
                                "elif".to_string()
                            } else {
                                format!("elif {cond_text}")
                            },
                            children,
                        });
                    }
                    if clause.kind() == "else_clause" {
                        let children =
                            collect_block(child_by_type(clause, "block"), class_name, self.src);
                        self.steps.push(CallStep::Branch {
                            key: "else".to_string(),
                            label: "else".to_string(),
                            children,
                        });
                    }
                }
                return;
            }
            "call" => {
                if let Some(callee) = node.named_child(0) {
                    if let Some(key) = callee_key(callee, class_name, self.src) {
                        self.add_call(key, node.start_byte());
                    }
                }
            }
            _ => {}
        }

        for child in named_children(node) {
            self.walk(child, class_name);
        }
    }
}

fn collect_statements(statements: Vec<Node>, class_name: Option<&str>, src: &str) -> Vec<CallStep> {
    let mut collector = Collector {
        src,
        steps: Vec::new(),
        seen: std::collections::HashSet::new(),
    };
    for stmt in statements {
        collector.walk(stmt, class_name);
    }
    collector.steps
}

fn handle_function(
    file: &str,
    node: Node,
    exported: bool,
    class_name: Option<&str>,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let Some(name_node) = child_by_type(node, "identifier") else {
        return;
    };
    let name = text(name_node, src).to_string();
    // skip dunder unless __init__
    if name.starts_with("__") && name != "__init__" {
        return;
    }

    let params = child_by_type(node, "parameters");
    let body = child_by_type(node, "block");
    let is_init = name == "__init__";
    let key = match class_name {
        Some(class_name) => format!("{class_name}.{name}"),
        None => name.clone(),
    };
    let label = match (class_name, is_init) {
        (Some(class_name), true) => class_name.to_string(),
        _ => key.clone(),
    };
    let params_label = params_label(params, src);
    let is_private = !is_init && name.starts_with('_');

    functions.push(FunctionInfo {
        key,
        label: format!("{label}{params_label}"),
        file: file.to_string(),
        steps: collect_block(body, class_name, src),
        exported: exported && !is_private,
    });

    if let (Some(class_name), true) = (class_name, is_init) {
        functions.push(FunctionInfo {
            key: format!("new {class_name}"),
            label: format!("{class_name}()"),
            file: file.to_string(),
            steps: collect_block(body, Some(class_name), src),
            exported,
        });
    }
}

fn unwrap_function(node: Node) -> Option<Node> {
    match node.kind() {
        "function_definition" => Some(node),
        "decorated_definition" => child_by_type(node, "function_definition"),
        _ => None,
    }
}

fn handle_class(
    file: &str,
    node: Node,
    exported: bool,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let Some(name_node) = child_by_type(node, "identifier") else {
        return;
    };
    let class_name = text(name_node, src).to_string();
    let class_exported = exported && !class_name.starts_with('_');
    let Some(body) = child_by_type(node, "block") else {
        return;
    };
    for stmt in named_children(body) {
        if let Some(func) = unwrap_function(stmt) {
            handle_function(
                file,
                func,
                class_exported,
                Some(&class_name),
                functions,
                src,
            );
        }
    }
}

fn handle_lambda_assignment(
    file: &str,
    statement: Node,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let Some(assign) = child_by_type(statement, "assignment") else {
        return;
    };
    let Some(target) = assign.named_child(0) else {
        return;
    };
    let Some(lambda) = child_by_type(assign, "lambda") else {
        return;
    };
    if target.kind() != "identifier" {
        return;
    }

    let name = text(target, src).to_string();
    if name.starts_with("__") {
        return;
    }
    let params = child_by_type(lambda, "lambda_parameters");
    let body = named_children(lambda).into_iter().last();

    functions.push(FunctionInfo {
        label: format!("{name}{}", params_label(params, src)),
        exported: !name.starts_with('_'),
        key: name,
        file: file.to_string(),
        steps: body
            .map(|b| collect_statements(vec![b], None, src))
            .unwrap_or_default(),
    });
}

fn visit(file: &str, node: Node, functions: &mut Vec<FunctionInfo>, src: &str) {
    if let Some(func) = unwrap_function(node) {
        handle_function(file, func, true, None, functions, src);
        return;
    }
    match node.kind() {
        "class_definition" => handle_class(file, node, true, functions, src),
        "expression_statement" => handle_lambda_assignment(file, node, functions, src),
        _ => {}
    }
}

pub fn extract(file: &str, source: &str, tree: &Tree) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for stmt in named_children(tree.root_node()) {
        visit(file, stmt, &mut functions, source);
    }
    functions
}
