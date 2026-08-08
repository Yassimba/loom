//! Rust callable extraction (tree-sitter-rust): free functions, impl
//! methods (self receivers key as `Type.method`, associated functions as
//! `Type::name`), calls through identifiers, method calls, paths, and
//! macros; if/else chains; let bindings for the dataflow layer.

use tree_sitter::{Node, Tree};

use super::{arg_text, child_by_type, collapse_ws, consumed, doc_before, named_children, text};
use crate::types::{line_of, CallMeta, CallStep, FunctionInfo, TypeInfo};

fn params_label(params: Option<Node>, src: &str) -> String {
    let Some(params) = params else {
        return "()".to_string();
    };
    let mut names: Vec<String> = Vec::new();
    for p in named_children(params) {
        match p.kind() {
            "parameter" => {
                let name = p
                    .named_child(0)
                    .map(|pattern| collapse_ws(text(pattern, src)))
                    .unwrap_or_else(|| "_".into());
                names.push(name);
            }
            "self_parameter" => names.push("self".into()),
            _ => {}
        }
    }
    if names.is_empty() {
        "()".to_string()
    } else {
        format!("({})", names.join(", "))
    }
}

/// "(name: Type, …)" as written; None for self-only/untyped lists.
fn typed_signature(params: Option<Node>, src: &str) -> Option<String> {
    let params = params?;
    let mut parts: Vec<String> = Vec::new();
    let mut typed = false;
    for p in named_children(params) {
        match p.kind() {
            "parameter" => {
                let name = p
                    .named_child(0)
                    .map(|pattern| collapse_ws(text(pattern, src)))
                    .unwrap_or_else(|| "_".into());
                match p.child_by_field_name("type") {
                    Some(t) => {
                        typed = true;
                        parts.push(format!("{name}: {}", collapse_ws(text(t, src))));
                    }
                    None => parts.push(name),
                }
            }
            "self_parameter" => parts.push("self".into()),
            _ => {}
        }
    }
    typed.then(|| format!("({})", parts.join(", ")))
}

fn callee_key(node: Node, self_type: Option<&str>, src: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(text(node, src).to_string()),
        "scoped_identifier" => Some(collapse_ws(text(node, src))),
        "field_expression" => {
            let receiver = node.named_child(0)?;
            let field = node.named_child(1)?;
            let method = text(field, src);
            match receiver.kind() {
                "self" => self_type.map(|t| format!("{t}.{method}")),
                "identifier" => Some(format!("{}.{method}", text(receiver, src))),
                _ => Some(method.to_string()),
            }
        }
        _ => None,
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
        if self.seen.insert(mark) {
            self.steps.push(CallStep::Call {
                key,
                meta,
                count: 1,
            });
        }
    }

    fn call_meta(&mut self, call: Node) -> CallMeta {
        let args = child_by_type(call, "arguments")
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

    fn walk(&mut self, node: Node, self_type: Option<&str>) {
        match node.kind() {
            "function_item" | "closure_expression" | "impl_item" | "mod_item" => return,
            "if_expression" => {
                let condition = node.child_by_field_name("condition");
                let cond = condition
                    .map(|c| collapse_ws(text(c, self.src)))
                    .unwrap_or_default();
                let children = node
                    .child_by_field_name("consequence")
                    .map(|block| collect_statements(named_children(block), self_type, self.src))
                    .unwrap_or_default();
                self.steps.push(CallStep::Branch {
                    key: if cond.is_empty() {
                        "if".into()
                    } else {
                        format!("if:{cond}")
                    },
                    label: if cond.is_empty() {
                        "if".into()
                    } else {
                        format!("if {cond}")
                    },
                    children,
                });
                if let Some(alternative) = node.child_by_field_name("alternative") {
                    if let Some(inner) = named_children(alternative).into_iter().next() {
                        if inner.kind() == "if_expression" {
                            // else if: flatten one level, keyed like the others
                            let mut nested = Collector {
                                src: self.src,
                                steps: Vec::new(),
                                seen: std::collections::HashSet::new(),
                                bindings: Vec::new(),
                                binding_ctx: None,
                            };
                            nested.walk(inner, self_type);
                            for step in nested.steps {
                                if let CallStep::Branch {
                                    key,
                                    label,
                                    children,
                                } = step
                                {
                                    let key = key.replacen("if", "else-if", 1);
                                    let label = label.replacen("if", "else if", 1);
                                    self.steps.push(CallStep::Branch {
                                        key,
                                        label,
                                        children,
                                    });
                                } else {
                                    self.steps.push(step);
                                }
                            }
                        } else {
                            self.steps.push(CallStep::Branch {
                                key: "else".into(),
                                label: "else".into(),
                                children: collect_statements(
                                    named_children(inner),
                                    self_type,
                                    self.src,
                                ),
                            });
                        }
                    }
                }
                return;
            }
            "let_declaration" => {
                let name = node
                    .child_by_field_name("pattern")
                    .filter(|pattern| pattern.kind() == "identifier")
                    .map(|pattern| text(pattern, self.src).to_string());
                if let Some(name) = name {
                    self.binding_ctx = Some(name.clone());
                    if let Some(value) = node.child_by_field_name("value") {
                        self.walk(value, self_type);
                    }
                    self.binding_ctx = None;
                    self.bindings.push(name);
                    return;
                }
            }
            "call_expression" => {
                if let Some(callee) = node.named_child(0) {
                    if let Some(key) = callee_key(callee, self_type, self.src) {
                        let meta = self.call_meta(node);
                        self.add_call(key, node.start_byte(), meta);
                    }
                }
            }
            "macro_invocation" => {
                if let Some(name) = node.named_child(0) {
                    let key = format!("{}!", collapse_ws(text(name, self.src)));
                    self.add_call(key, node.start_byte(), CallMeta::default());
                }
            }
            _ => {}
        }

        for child in named_children(node) {
            self.walk(child, self_type);
        }
    }
}

fn collect_statements(statements: Vec<Node>, self_type: Option<&str>, src: &str) -> Vec<CallStep> {
    let mut collector = Collector {
        src,
        steps: Vec::new(),
        seen: std::collections::HashSet::new(),
        bindings: Vec::new(),
        binding_ctx: None,
    };
    for stmt in statements {
        collector.walk(stmt, self_type);
    }
    collector.steps
}

fn handle_function(
    file: &str,
    node: Node,
    self_type: Option<&str>,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let name = text(name_node, src).to_string();
    let params = node.child_by_field_name("parameters");
    let has_self = params
        .map(|list| {
            named_children(list)
                .iter()
                .any(|p| p.kind() == "self_parameter")
        })
        .unwrap_or(false);
    let key = match (self_type, has_self) {
        (Some(t), true) => format!("{t}.{name}"),
        (Some(t), false) => format!("{t}::{name}"),
        (None, _) => name.clone(),
    };
    let body = node.child_by_field_name("body");
    // `pub` visibility marks exported; methods inherit from being reachable.
    let exported = node
        .named_child(0)
        .map(|first| first.kind() == "visibility_modifier")
        .unwrap_or(false)
        || self_type.is_some();
    functions.push(FunctionInfo {
        label: format!("{key}{}", params_label(params, src)),
        key,
        file: file.to_string(),
        line: line_of(src, node.start_byte()),
        signature: typed_signature(params, src),
        doc: doc_before(node, src),
        returns: node
            .child_by_field_name("return_type")
            .map(|ret| collapse_ws(text(ret, src))),
        steps: body
            .map(|block| collect_statements(named_children(block), self_type, src))
            .unwrap_or_default(),
        exported,
    });
}

fn visit(file: &str, node: Node, functions: &mut Vec<FunctionInfo>, src: &str) {
    match node.kind() {
        "function_item" => handle_function(file, node, None, functions, src),
        "impl_item" => {
            let self_type = node
                .child_by_field_name("type")
                .map(|t| collapse_ws(text(t, src)));
            if let Some(body) = node.child_by_field_name("body") {
                for item in named_children(body) {
                    if item.kind() == "function_item" {
                        handle_function(file, item, self_type.as_deref(), functions, src);
                    }
                }
            }
        }
        "mod_item" => {
            if let Some(body) = node.child_by_field_name("body") {
                for item in named_children(body) {
                    visit(file, item, functions, src);
                }
            }
        }
        _ => {}
    }
}

pub fn extract(file: &str, source: &str, tree: &Tree) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for item in named_children(tree.root_node()) {
        visit(file, item, &mut functions, source);
    }
    functions
}

/// Struct fields for the --er view.
pub fn extract_types(source: &str, tree: &Tree) -> Vec<TypeInfo> {
    let mut types = Vec::new();
    fn visit(node: Node, types: &mut Vec<TypeInfo>, src: &str) {
        if node.kind() == "struct_item" {
            if let Some(name) = node.child_by_field_name("name") {
                let mut fields = Vec::new();
                if let Some(body) = node.child_by_field_name("body") {
                    for field in named_children(body) {
                        if field.kind() == "field_declaration" {
                            if let (Some(field_name), Some(ty)) = (
                                field.child_by_field_name("name"),
                                field.child_by_field_name("type"),
                            ) {
                                fields.push((
                                    text(field_name, src).to_string(),
                                    Some(collapse_ws(text(ty, src))),
                                ));
                            }
                        }
                    }
                }
                types.push(TypeInfo {
                    name: text(name, src).to_string(),
                    fields,
                });
            }
        }
        if matches!(node.kind(), "mod_item") {
            if let Some(body) = node.child_by_field_name("body") {
                for child in named_children(body) {
                    visit(child, types, src);
                }
            }
        }
    }
    for item in named_children(tree.root_node()) {
        visit(item, &mut types, source);
    }
    types
}
