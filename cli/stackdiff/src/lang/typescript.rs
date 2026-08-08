//! TypeScript / TSX callable extraction (tree-sitter-typescript).
//! Faithful port of calldiff's typescript.ts.

use tree_sitter::{Node, Tree};

use super::{child_by_type, collapse_ws, named_children, text};
use crate::types::{CallStep, FunctionInfo};

fn is_fn_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_expression"
            | "arrow_function"
            | "generator_function"
            | "generator_function_declaration"
            | "method_definition"
    )
}

fn params_label(params: Option<Node>, src: &str) -> String {
    let Some(params) = params else {
        return "()".to_string();
    };
    if params.kind() != "formal_parameters" {
        return "()".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for p in named_children(params) {
        match p.kind() {
            "required_parameter" | "optional_parameter" => {
                if let Some(rest) = child_by_type(p, "rest_pattern") {
                    let id = child_by_type(rest, "identifier");
                    parts.push(match id {
                        Some(id) => format!("...{}", text(id, src)),
                        None => "...".to_string(),
                    });
                    continue;
                }
                if let Some(id) = child_by_type(p, "identifier") {
                    parts.push(text(id, src).to_string());
                    continue;
                }
                if child_by_type(p, "object_pattern").is_some() {
                    parts.push("{}".to_string());
                    continue;
                }
                if child_by_type(p, "array_pattern").is_some() {
                    parts.push("[]".to_string());
                    continue;
                }
                parts.push("_".to_string());
            }
            "rest_parameter" | "rest_pattern" => {
                let id = child_by_type(p, "identifier");
                parts.push(match id {
                    Some(id) => format!("...{}", text(id, src)),
                    None => "...".to_string(),
                });
            }
            _ => parts.push("_".to_string()),
        }
    }
    if parts.is_empty() {
        "()".to_string()
    } else {
        format!("({})", parts.join(", "))
    }
}

fn cond_text(test: Node, src: &str) -> String {
    if test.kind() == "parenthesized_expression" {
        if let Some(inner) = test.named_child(0) {
            return collapse_ws(text(inner, src));
        }
    }
    collapse_ws(text(test, src))
}

fn branch_key(kind: &str, cond: &str) -> String {
    if kind == "else" {
        "else".to_string()
    } else {
        format!("{kind}:{cond}")
    }
}

fn callee_key(node: Node, class_name: Option<&str>, src: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(text(node, src).to_string()),
        "this" => class_name.map(|c| c.to_string()),
        "subscript_expression" => None,
        "member_expression" => {
            let object = node.named_child(0)?;
            let property = named_children(node).into_iter().find(|c| {
                c.kind() == "property_identifier" || c.kind() == "private_property_identifier"
            })?;
            let prop = text(property, src);
            if object.kind() == "this" {
                if let Some(class_name) = class_name {
                    return Some(format!("{class_name}.{prop}"));
                }
            }
            if object.kind() == "identifier" {
                return Some(format!("{}.{prop}", text(object, src)));
            }
            if let Some(class_name) = class_name {
                return Some(format!("{class_name}.{prop}"));
            }
            Some(prop.to_string())
        }
        _ => None,
    }
}

fn statements_of(node: Node) -> Vec<Node> {
    if node.kind() == "statement_block" {
        named_children(node)
    } else {
        vec![node]
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
        let kind = node.kind();

        if is_fn_like(kind) && kind != "method_definition" {
            return;
        }

        if kind == "if_statement" {
            let kids = named_children(node);
            let test = child_by_type(node, "parenthesized_expression")
                .or_else(|| kids.iter().copied().find(|c| c.kind() != "else_clause"));
            let consequent = kids
                .iter()
                .copied()
                .find(|c| c.kind() != "parenthesized_expression" && c.kind() != "else_clause");
            let else_clause = child_by_type(node, "else_clause");
            let cond = test.map(|t| cond_text(t, self.src)).unwrap_or_default();

            let children = consequent
                .map(|c| collect_statements(statements_of(c), class_name, self.src))
                .unwrap_or_default();
            self.steps.push(CallStep::Branch {
                key: branch_key("if", &cond),
                label: match test {
                    Some(t) => format!("if ({})", cond_text(t, self.src)),
                    None => "if".to_string(),
                },
                children,
            });

            let mut current = else_clause;
            while let Some(clause) = current {
                let Some(inner) = clause.named_child(0) else {
                    break;
                };

                if inner.kind() == "if_statement" {
                    let else_test = child_by_type(inner, "parenthesized_expression");
                    let else_kids = named_children(inner);
                    let else_consequent = else_kids.iter().copied().find(|c| {
                        c.kind() != "parenthesized_expression" && c.kind() != "else_clause"
                    });
                    let else_cond = else_test
                        .map(|t| cond_text(t, self.src))
                        .unwrap_or_default();
                    let children = else_consequent
                        .map(|c| collect_statements(statements_of(c), class_name, self.src))
                        .unwrap_or_default();
                    self.steps.push(CallStep::Branch {
                        key: branch_key("else-if", &else_cond),
                        label: match else_test {
                            Some(t) => format!("else if ({})", cond_text(t, self.src)),
                            None => "else if".to_string(),
                        },
                        children,
                    });
                    current = child_by_type(inner, "else_clause");
                    continue;
                }

                self.steps.push(CallStep::Branch {
                    key: branch_key("else", ""),
                    label: "else".to_string(),
                    children: collect_statements(statements_of(inner), class_name, self.src),
                });
                break;
            }
            return;
        }

        if kind == "call_expression" {
            if let Some(callee) = node.named_child(0) {
                if let Some(key) = callee_key(callee, class_name, self.src) {
                    self.add_call(key, node.start_byte());
                }
            }
        } else if kind == "new_expression" {
            if let Some(callee) = node.named_child(0) {
                if let Some(key) = callee_key(callee, None, self.src) {
                    let key = if key.starts_with("new ") {
                        key
                    } else {
                        format!("new {key}")
                    };
                    self.add_call(key, node.start_byte());
                }
            }
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

fn steps_from_body(body: Option<Node>, class_name: Option<&str>, src: &str) -> Vec<CallStep> {
    let Some(body) = body else {
        return Vec::new();
    };
    if body.kind() == "statement_block" {
        collect_statements(named_children(body), class_name, src)
    } else {
        collect_statements(vec![body], class_name, src)
    }
}

#[allow(clippy::too_many_arguments)]
fn function_from_parts(
    file: &str,
    key: String,
    label: &str,
    params: Option<Node>,
    body: Option<Node>,
    exported: bool,
    class_name: Option<&str>,
    src: &str,
) -> FunctionInfo {
    let params_label = params_label(params, src);
    FunctionInfo {
        label: format!("{label}{params_label}"),
        key,
        file: file.to_string(),
        steps: steps_from_body(body, class_name, src),
        exported,
    }
}

fn handle_function_node(
    file: &str,
    node: Node,
    name: Option<String>,
    exported: bool,
    class_name: Option<&str>,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let Some(name) = name else { return };
    let key = match class_name {
        Some(class_name) => format!("{class_name}.{name}"),
        None => name,
    };
    let params = child_by_type(node, "formal_parameters");
    let body = child_by_type(node, "statement_block").or_else(|| {
        named_children(node).into_iter().find(|c| {
            !matches!(
                c.kind(),
                "formal_parameters"
                    | "type_parameters"
                    | "type_annotation"
                    | "identifier"
                    | "accessibility_modifier"
                    | "async"
                    | "readonly"
            )
        })
    });

    functions.push(function_from_parts(
        file,
        key.clone(),
        &key,
        params,
        body,
        exported,
        class_name,
        src,
    ));
}

fn handle_class(
    file: &str,
    node: Node,
    exported: bool,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    let name_node =
        child_by_type(node, "type_identifier").or_else(|| child_by_type(node, "identifier"));
    let Some(name_node) = name_node else { return };
    let class_name = text(name_node, src).to_string();

    let Some(body) = child_by_type(node, "class_body") else {
        return;
    };

    for element in named_children(body) {
        if element.kind() == "method_definition" {
            let key_node = child_by_type(element, "property_identifier")
                .or_else(|| child_by_type(element, "private_property_identifier"))
                .or_else(|| child_by_type(element, "computed_property_name"));
            let Some(key_node) = key_node else { continue };
            let method_name = text(key_node, src).to_string();
            let is_constructor = method_name == "constructor";

            let accessibility = child_by_type(element, "accessibility_modifier");
            let method_exported = exported
                || accessibility
                    .map(|a| text(a, src) == "public")
                    .unwrap_or(false);

            let params = child_by_type(element, "formal_parameters");
            let fn_body = child_by_type(element, "statement_block");
            let key = if is_constructor {
                format!("{class_name}.constructor")
            } else {
                format!("{class_name}.{method_name}")
            };
            let label = if is_constructor {
                format!("new {class_name}()")
            } else {
                key.clone()
            };

            functions.push(function_from_parts(
                file,
                key,
                &label,
                params,
                fn_body,
                method_exported,
                Some(&class_name),
                src,
            ));
        }

        if element.kind() == "public_field_definition" {
            let key_node = child_by_type(element, "property_identifier");
            let value = child_by_type(element, "arrow_function")
                .or_else(|| child_by_type(element, "function_expression"));
            if let (Some(key_node), Some(value)) = (key_node, value) {
                handle_function_node(
                    file,
                    value,
                    Some(text(key_node, src).to_string()),
                    exported,
                    Some(&class_name),
                    functions,
                    src,
                );
            }
        }
    }
}

fn visit_statement(
    file: &str,
    node: Node,
    exported: bool,
    functions: &mut Vec<FunctionInfo>,
    src: &str,
) {
    match node.kind() {
        "export_statement" => {
            let decl = named_children(node)
                .into_iter()
                .find(|c| c.kind() != "export_clause");
            let Some(decl) = decl else { return };

            let is_default = text(node, src).starts_with("export default");

            match decl.kind() {
                "function_declaration"
                | "function_expression"
                | "generator_function_declaration"
                | "generator_function" => {
                    let id = child_by_type(decl, "identifier");
                    let name = id
                        .map(|id| text(id, src).to_string())
                        .or_else(|| is_default.then(|| "default".to_string()));
                    handle_function_node(file, decl, name, true, None, functions, src);
                }
                "arrow_function" => {
                    handle_function_node(
                        file,
                        decl,
                        is_default.then(|| "default".to_string()),
                        true,
                        None,
                        functions,
                        src,
                    );
                }
                "class_declaration" | "abstract_class_declaration" | "class" => {
                    handle_class(file, decl, true, functions, src);
                }
                "lexical_declaration" | "variable_declaration" => {
                    visit_statement(file, decl, true, functions, src);
                }
                _ => {}
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            let id = child_by_type(node, "identifier");
            handle_function_node(
                file,
                node,
                id.map(|id| text(id, src).to_string()),
                exported,
                None,
                functions,
                src,
            );
        }
        "class_declaration" | "abstract_class_declaration" => {
            handle_class(file, node, exported, functions, src);
        }
        "lexical_declaration" | "variable_declaration" => {
            for d in named_children(node) {
                if d.kind() != "variable_declarator" {
                    continue;
                }
                let id = child_by_type(d, "identifier");
                let init = child_by_type(d, "arrow_function")
                    .or_else(|| child_by_type(d, "function_expression"));
                if let (Some(id), Some(init)) = (id, init) {
                    handle_function_node(
                        file,
                        init,
                        Some(text(id, src).to_string()),
                        exported,
                        None,
                        functions,
                        src,
                    );
                }
            }
        }
        _ => {}
    }
}

pub fn extract(file: &str, source: &str, tree: &Tree) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for stmt in named_children(tree.root_node()) {
        visit_statement(file, stmt, false, &mut functions, source);
    }
    functions
}
