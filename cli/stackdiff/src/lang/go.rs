//! Go callable extraction (tree-sitter-go).
//! Faithful port of calldiff's go.ts.

use tree_sitter::{Node, Tree};

use crate::types::TypeInfo;

use super::{arg_text, child_by_type, collapse_ws, consumed, doc_before, named_children, text};
use crate::types::{line_of, CallMeta, CallStep, FunctionInfo};

fn is_exported(name: &str) -> bool {
    name.chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
}

fn params_label(params: Option<Node>, src: &str) -> String {
    let Some(params) = params else {
        return "()".to_string();
    };
    if params.kind() != "parameter_list" {
        return "()".to_string();
    }
    let mut names: Vec<String> = Vec::new();
    for p in named_children(params) {
        if p.kind() != "parameter_declaration" {
            continue;
        }
        let id = child_by_type(p, "identifier");
        names.push(
            id.map(|id| text(id, src).to_string())
                .unwrap_or_else(|| "_".to_string()),
        );
    }
    if names.is_empty() {
        "()".to_string()
    } else {
        format!("({})", names.join(", "))
    }
}

/// "(name Type, …)" as written; None when untyped/unnamed.
fn typed_signature(params: Option<Node>, src: &str) -> Option<String> {
    let params = params.filter(|p| p.kind() == "parameter_list")?;
    let mut parts: Vec<String> = Vec::new();
    let mut typed = false;
    for p in named_children(params) {
        if p.kind() != "parameter_declaration" {
            continue;
        }
        let ty = p
            .child_by_field_name("type")
            .map(|t| collapse_ws(text(t, src)));
        let name = child_by_type(p, "identifier").map(|id| text(id, src).to_string());
        match (name, ty) {
            (Some(name), Some(ty)) => {
                typed = true;
                parts.push(format!("{name} {ty}"));
            }
            (Some(name), None) => parts.push(name),
            (None, Some(ty)) => {
                typed = true;
                parts.push(ty);
            }
            (None, None) => parts.push("_".to_string()),
        }
    }
    typed.then(|| format!("({})", parts.join(", ")))
}

fn callee_key(node: Node, receiver_type: Option<&str>, src: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(text(node, src).to_string()),
        "selector_expression" => {
            let object = node.named_child(0)?;
            let field = child_by_type(node, "field_identifier")?;
            let prop = text(field, src);
            if object.kind() == "identifier" {
                let obj_name = text(object, src);
                // Heuristic: lowercase receiver var → Type.Method when in a method
                if let Some(receiver_type) = receiver_type {
                    if obj_name
                        .chars()
                        .next()
                        .map(|c| c.is_lowercase())
                        .unwrap_or(false)
                    {
                        return Some(format!("{receiver_type}.{prop}"));
                    }
                }
                return Some(format!("{obj_name}.{prop}"));
            }
            if let Some(receiver_type) = receiver_type {
                return Some(format!("{receiver_type}.{prop}"));
            }
            Some(prop.to_string())
        }
        _ => None,
    }
}

fn statements_of(node: Node) -> Vec<Node> {
    match node.kind() {
        "block" => match child_by_type(node, "statement_list") {
            Some(list) => named_children(list),
            None => named_children(node),
        },
        "statement_list" => named_children(node),
        _ => vec![node],
    }
}

struct Collector<'s> {
    src: &'s str,
    steps: Vec<CallStep>,
    seen: std::collections::HashSet<String>,
    bindings: Vec<String>,
    binding_ctx: Option<String>,
}

impl<'s> Collector<'s> {
    fn add_call(&mut self, key: String, start: usize, meta: CallMeta) {
        let mark = format!("{key}:{start}");
        if self.seen.contains(&mark) {
            return;
        }
        self.seen.insert(mark);
        self.steps.push(CallStep::Call {
            key,
            meta,
            count: 1,
        });
    }

    fn call_meta(&mut self, call: Node) -> CallMeta {
        let args = child_by_type(call, "argument_list")
            .map(|arguments| {
                named_children(arguments)
                    .into_iter()
                    .map(|arg| arg_text(arg, self.src))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        CallMeta {
            binding: self.binding_ctx.take(),
            consumes: consumed(&self.bindings, &args),
            args,
        }
    }

    fn walk(&mut self, node: Node, receiver_type: Option<&str>) {
        match node.kind() {
            "function_declaration" | "method_declaration" => return,
            "if_statement" => {
                let kids = named_children(node);
                let cond = kids.iter().copied().find(|c| c.kind() != "block");
                let blocks: Vec<Node> = kids
                    .iter()
                    .copied()
                    .filter(|c| c.kind() == "block")
                    .collect();
                let cond_text = cond
                    .map(|c| collapse_ws(text(c, self.src)))
                    .unwrap_or_default();
                let children = blocks
                    .first()
                    .map(|b| collect_statements(statements_of(*b), receiver_type, self.src))
                    .unwrap_or_default();
                self.steps.push(CallStep::Branch {
                    key: if cond_text.is_empty() {
                        "if".to_string()
                    } else {
                        format!("if:{cond_text}")
                    },
                    label: if cond_text.is_empty() {
                        "if".to_string()
                    } else {
                        format!("if {cond_text}")
                    },
                    children,
                });
                if let Some(else_block) = blocks.get(1) {
                    let children =
                        collect_statements(statements_of(*else_block), receiver_type, self.src);
                    self.steps.push(CallStep::Branch {
                        key: "else".to_string(),
                        label: "else".to_string(),
                        children,
                    });
                }
                return;
            }
            "short_var_declaration" | "assignment_statement" => {
                let name = node
                    .named_child(0)
                    .and_then(|left| named_children(left).into_iter().next())
                    .filter(|first| first.kind() == "identifier")
                    .map(|first| text(first, self.src).to_string());
                if let Some(name) = name {
                    self.binding_ctx = Some(name.clone());
                    for child in named_children(node).into_iter().skip(1) {
                        self.walk(child, receiver_type);
                    }
                    self.binding_ctx = None;
                    self.bindings.push(name);
                    return;
                }
            }
            "call_expression" => {
                if let Some(callee) = node.named_child(0) {
                    if let Some(key) = callee_key(callee, receiver_type, self.src) {
                        let meta = self.call_meta(node);
                        self.add_call(key, node.start_byte(), meta);
                    }
                }
            }
            _ => {}
        }

        for child in named_children(node) {
            self.walk(child, receiver_type);
        }
    }
}

fn collect_statements(
    statements: Vec<Node>,
    receiver_type: Option<&str>,
    src: &str,
) -> Vec<CallStep> {
    let mut collector = Collector {
        src,
        steps: Vec::new(),
        seen: std::collections::HashSet::new(),
        bindings: Vec::new(),
        binding_ctx: None,
    };
    for stmt in statements {
        collector.walk(stmt, receiver_type);
    }
    collector.steps
}

fn receiver_type_name(method: Node, src: &str) -> Option<String> {
    let recv = method.named_child(0)?;
    if recv.kind() != "parameter_list" {
        return None;
    }
    let decl = child_by_type(recv, "parameter_declaration")?;
    let pointer = child_by_type(decl, "pointer_type");
    let type_id = match pointer {
        Some(pointer) => child_by_type(pointer, "type_identifier")
            .or_else(|| child_by_type(decl, "type_identifier")),
        None => child_by_type(decl, "type_identifier"),
    };
    type_id.map(|t| text(t, src).to_string())
}

fn handle_function(file: &str, node: Node, functions: &mut Vec<FunctionInfo>, src: &str) {
    let Some(name_node) = child_by_type(node, "identifier") else {
        return;
    };
    let name = text(name_node, src).to_string();
    let params = named_children(node)
        .into_iter()
        .find(|c| c.kind() == "parameter_list");
    let body = child_by_type(node, "block");
    functions.push(FunctionInfo {
        label: format!("{name}{}", params_label(params, src)),
        exported: is_exported(&name),
        key: name,
        file: file.to_string(),
        line: line_of(src, node.start_byte()),
        doc: doc_before(node, src),
        returns: go_result(node, src),
        signature: typed_signature(params, src),
        steps: body
            .map(|b| collect_statements(statements_of(b), None, src))
            .unwrap_or_default(),
    });
}

/// The declared result type: the named child between params and body.
fn go_result(node: Node, src: &str) -> Option<String> {
    node.child_by_field_name("result")
        .map(|result| collapse_ws(text(result, src)))
}

fn handle_method(file: &str, node: Node, functions: &mut Vec<FunctionInfo>, src: &str) {
    let Some(type_name) = receiver_type_name(node, src) else {
        return;
    };
    let Some(name_node) = child_by_type(node, "field_identifier") else {
        return;
    };
    let name = text(name_node, src).to_string();

    // parameter_list after receiver
    let param_lists: Vec<Node> = named_children(node)
        .into_iter()
        .filter(|c| c.kind() == "parameter_list")
        .collect();
    let params = param_lists.get(1).or(param_lists.first()).copied();
    let body = child_by_type(node, "block");
    let key = format!("{type_name}.{name}");

    functions.push(FunctionInfo {
        label: format!("{key}{}", params_label(params, src)),
        key,
        file: file.to_string(),
        line: line_of(src, node.start_byte()),
        doc: doc_before(node, src),
        returns: go_result(node, src),
        signature: typed_signature(params, src),
        steps: body
            .map(|b| collect_statements(statements_of(b), Some(&type_name), src))
            .unwrap_or_default(),
        exported: is_exported(&name),
    });
}

pub fn extract(file: &str, source: &str, tree: &Tree) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for stmt in named_children(tree.root_node()) {
        match stmt.kind() {
            "function_declaration" => handle_function(file, stmt, &mut functions, source),
            "method_declaration" => handle_method(file, stmt, &mut functions, source),
            _ => {}
        }
    }
    functions
}

/// Struct fields for the --er view.
pub fn extract_types(source: &str, tree: &Tree) -> Vec<TypeInfo> {
    let mut types = Vec::new();
    for node in named_children(tree.root_node()) {
        if node.kind() != "type_declaration" {
            continue;
        }
        for spec in named_children(node) {
            if spec.kind() != "type_spec" {
                continue;
            }
            let (Some(name), Some(body)) = (
                spec.child_by_field_name("name"),
                spec.child_by_field_name("type"),
            ) else {
                continue;
            };
            if body.kind() != "struct_type" {
                continue;
            }
            let mut fields = Vec::new();
            for list in named_children(body) {
                for field in named_children(list) {
                    if field.kind() == "field_declaration" {
                        let ty = field
                            .child_by_field_name("type")
                            .map(|t| collapse_ws(text(t, source)));
                        for id in named_children(field) {
                            if id.kind() == "field_identifier" {
                                fields.push((text(id, source).to_string(), ty.clone()));
                            }
                        }
                    }
                }
            }
            types.push(TypeInfo {
                name: text(name, source).to_string(),
                fields,
            });
        }
    }
    types
}
