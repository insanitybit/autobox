#![allow(unused_imports, dead_code, unreachable_code, unused_variables)]

use autobox_effect_parser::ast::{DeclareMacro, Expr};

use eyre::{eyre, Report, Result, WrapErr};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use syn::__private::ToTokens;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ExprPath, ItemFn, Local};
use walkdir::WalkDir;

fn read_ast(path: &std::path::Path) -> Result<syn::File> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    Ok(syn::parse_file(&content)?)
}

fn check_if_declare(item_fn: &ItemFn) -> bool {
    for attr in &item_fn.attrs {
        if let Some(last_segment) = attr.path.segments.last() {
            if last_segment.ident.to_string() == "declare" {
                return true;
            }
        }
    }
    false
}

fn check_if_entrypoint(item_fn: &ItemFn) -> bool {
    for attr in &item_fn.attrs {
        if let Some(last_segment) = attr.path.segments.last() {
            if last_segment.ident.to_string() == "entrypoint" {
                return true;
            }
        }
    }
    false
}

fn find_entrypoint(ast: &syn::File) -> Option<&ItemFn> {
    for item in &ast.items {
        match item {
            syn::Item::Fn(f) => {
                if check_if_entrypoint(&f) {
                    return Some(f);
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Clone, Debug, Default)]
struct ItemFnVisitor<'ast> {
    item_fns: Vec<&'ast syn::ItemFn>,
}

impl<'ast> ItemFnVisitor<'ast> {
    fn collect_from_ast(ast: &'ast syn::File) -> Self {
        let mut visitor = ItemFnVisitor::default();
        visitor.visit_file(ast);
        visitor
    }
}

impl<'ast> Visit<'ast> for ItemFnVisitor<'ast> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.item_fns.push(node);
        visit::visit_item_fn(self, node);
    }
}

#[derive(Clone, Debug, Default)]
struct FnCallVisitor<'ast> {
    fn_calls: Vec<&'ast syn::ExprCall>,
}

impl<'ast> Visit<'ast> for FnCallVisitor<'ast> {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        self.fn_calls.push(node);
        visit::visit_expr_call(self, node);
    }
}

fn get_function_calls(item_fn: &ItemFn) -> Vec<&syn::ExprCall> {
    let mut fn_call_visitor = FnCallVisitor::default();
    fn_call_visitor.visit_item_fn(&item_fn);
    fn_call_visitor.fn_calls
}

fn get_fn_name_from_call(call: &ExprCall) -> String {
    match &*call.func {
        syn::Expr::Path(path) => path.path.segments.last().unwrap().ident.to_string(),
        _ => {
            panic!("Not a path")
        }
    }
}

fn remove_surrounding<'a>(s: &'a str, r: &str) -> &'a str {
    let s = s.strip_prefix(r).unwrap_or(s);
    let r = match r {
        "(" => ")",
        r => r,
    };
    s.strip_suffix(r).unwrap_or(s)
}


// Resolves arguments to a call
// If an argument is unknown it becomes "*" (terrible)
fn resolve_arguments(call: ExprCall) -> Vec<String> {
    // Inline literals are the trivial case, otherwise
    // we need to be able to go from a variable name
    // back to its source
    // example:
    //
    // ```
    // let x = "foo";
    // let y = x;
    // fn_with_effects(y);
    // ```
    // We need to track y back to x, then x back to "foo"
    // and if we can't track it back we need to use "*"
    // also, "*" is a terrible marker for this, we need a way
    // to express that a resolved argument is made up of both
    // known and unknown values but "*" is ez mode now

    let mut resolved_args = Vec::new();
    for arg in call.args {
        match arg {
            // extract literal
            syn::Expr::Lit(path) => {
                let path = path.to_token_stream().to_string();
                resolved_args.push(path);
            }
            // We only support inlined literals for now ok
            _ => {
                resolved_args.push("*".to_string());
            }
        }
    }
    resolved_args
}

// A Visitor impl that collects a "trace" of the path from a function's start
// to the targeted function call
struct TracingVisitor<'a> {
    fn_call: &'a ExprCall,
    arg_to_trace: u8, // the index of the argument we want to trace
}

impl<'ast, 'a> Visit<'ast> for TracingVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        // This is where our trace begins, we need to trace the entire ast
        // from this point until the call, building up
    }
}


// This program should print out:
// Main requires `read_file("~/filename.txt")`
fn main() -> Result<()> {
    println!(
        "running from: {}",
        std::env::current_dir().unwrap().display()
    );

    // for entry in WalkDir::new("./example-app/src/") {

    // First we find the entrypoint, then we find all function calls,
    let ast = read_ast(Path::new("./example-app/src/main.rs"))?;
    let entrypoint = find_entrypoint(&ast).unwrap();

    let entrypoint_fn_calls = get_function_calls(entrypoint);

    // Then we find all functions that have side effects
    let declared_item_fns: HashMap<String, &ItemFn> = ItemFnVisitor::collect_from_ast(&ast)
        .item_fns
        .into_iter()
        .flat_map(|f| {
            if check_if_declare(&f) {
                Some((f.sig.ident.to_string(), f))
            } else {
                None
            }
        })
        .collect();

    // The declared functions that are actually called in main
    let mut declared_fn_calls: HashMap<String, &ExprCall> = HashMap::new();
    for call in entrypoint_fn_calls.iter() {
        let call_name = get_fn_name_from_call(call);
        if let Some(_) = declared_item_fns.get(&call_name) {
            declared_fn_calls.insert(call_name, call);
        }
    }

    // let mut declarations = HashMap::new();

    for name in declared_fn_calls.keys() {
        // todo: handle more than one attribute
        let attribute = &declared_item_fns[&name[..]].attrs[0];
        // let declaration = DeclareAttribute::from_attribute(attribute);
        // declarations.insert(name.clone(), declaration);
    }

    // let mut resolved_side_effects = Vec::new();
    // // Find the calls, evaluate them given their arguments
    // for (name, call) in declared_fn_calls {
    //     // todo: for now we don't do any fancy resolution of values, we just
    //     // assume they are static
    //     // retrieve the arguments to the call
    //
    //     let arguments = resolve_arguments(call.clone());
    //     let declaration = declarations.get(&name[..]).unwrap();
    //     resolved_side_effects.extend_from_slice(&declaration.execute(arguments)[..]);
    // }
    //
    // println!("resolved_side_effects: {:?}", resolved_side_effects);

    // and now we can generate policy

    Ok(())
}


#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use super::*;

    #[test]
    fn trace_var() {
        let rust_code = r#"
        #[effect::declare(
            args=(a as A, b as B),
            side_effects=(read_file(a)),
            returns=(A + '/' + B),
        )]
        fn fn_with_effects(a: &str, b: &str) {
            // pretend there are effects here
            format!("{a}/{b}")
        }

        fn unknown() -> &str {
            "foo"
        }

        #[effect::entrypoint]
        fn main() {
            let x = "foo";
            let y = x;
            fn_with_effects(y, "bar");
            let uk = unknown();
            let z = fn_with_effects(y, uk);
        }
        "#;

        let ast = syn::parse_file(rust_code).unwrap();
        let entrypoint = find_entrypoint(&ast).unwrap();
        let all_declared_fns = get_all_declared_fns(&ast);

        // `variables` is a mapping of (i, variable_name) -> VariableMetadata
        // where `i` is the instance number of that variable, allowing us to differentiate
        // between variables with the same name (due to shadowing)
        let mut variables = BTreeMap::new();

        // We're going to track all 'let' statements, and attach metadata
        // about them to the variable name
        for (i, statement) in entrypoint.block.stmts.iter().enumerate() {
            let i = i as u16;  // dont put > 2^16 statements in your code!!!
            println!("i: {i:#?}");
            match statement {
                syn::Stmt::Local(local) => {
                    // for now we only support single Ident bindings
                    let var_name = extract_variables_from_pat(&local.pat)[0];
                    variables.insert(
                        (i, var_name),
                        VariableMetadata::new(
                            var_name,
                            i as u16,
                            local,
                            get_variable_state(&local.init.as_ref().unwrap().1.as_ref(), i, &variables, &all_declared_fns),
                        )
                    );
                }
                syn::Stmt::Semi(syn::Expr::Call(fn_call), _) => {
                    // println!("fn_call: {:#?}", fn_call);
                    for arg in fn_call.args.iter() {
                        // Figure out what we know about the arguments
                        match arg {
                            syn::Expr::Path(path) => {
                                let var_name = &path.path.segments[0].ident;
                                let arg_var = find_variable_metadata(var_name, i as u16, &variables).unwrap();
                                println!("arg_var: {:#?}", arg_var);

                            }
                            syn::Expr::Lit(lit) => {
                                println!("lit: {lit:#?}");


                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        println!("variables: {:#?}", variables);
    }
}

#[derive(Debug, Clone)]
pub struct DeclaredItemFn<'a> {
    item_fn: &'a ItemFn,
    declaration: DeclareMacro<'a>,
}


/// find all functions that are marked `declare`
fn get_all_declared_fns<'a>(ast: &'a syn::File) -> HashMap<String, DeclaredItemFn<'a>> {
    ItemFnVisitor::collect_from_ast(&ast)
        .item_fns
        .into_iter()
        .flat_map(|f| {
            if check_if_declare(&f) {
                // todo: support multiple attrs
                // todo: Yeah yeah I leak it whatever
                let macro_tokens = Box::leak(f.attrs[0].tokens.to_string().into_boxed_str());
                let (_, declaration) = DeclareMacro::parse(&*macro_tokens).unwrap();
                Some((f.sig.ident.to_string(), DeclaredItemFn {
                    item_fn: f,
                    declaration,
                }))
            } else {
                None
            }
        })
        .collect()
}

fn get_variable_state(
    expression: &syn::Expr,
    index: u16,
    variables: &BTreeMap<(u16, &syn::Ident), VariableMetadata>,
    all_declared_fns: &HashMap<String, DeclaredItemFn<'_>>,
) -> VariableState {
    match expression {
        syn::Expr::Path(ref path) => {
            let var_name = &path.path.segments[0].ident;
            let var = find_variable_metadata(var_name, index, variables).unwrap();
            var.variable_state.clone()
        }
        syn::Expr::Lit(ref lit) => {
            match lit.lit {
                syn::Lit::Str(ref s) => VariableState::value(s.value()),
                _ => {
                    println!("unsupported literal type for: {:#?}", expression);
                    VariableState::hole()
                },
            }
        }
        syn::Expr::Call(ref call) => {
            let fn_name = match call.func.as_ref() {
                syn::Expr::Path(ref path) => &path.path.segments[0].ident,
                _ => {
                    panic!("unsupported function call: {:#?}", expression);
                }
            };
            let fn_item = match all_declared_fns.get(&fn_name.to_string()) {
                Some(f) => f,
                None => {
                    // We have no insight into this function
                    return VariableState::hole();
                }
            };
            // Given a call we need to calculate the return value
            // based on its inputs
            let mut arg_states = Vec::new();
            for arg in call.args.iter() {
                arg_states.push(get_variable_state(arg, index, variables, all_declared_fns));
            }
            // Now we have to calculate the state of the return value

            evaluate_declared_fn(fn_item, arg_states)
        }
        _ => {
            println!("Unsupported expression type for: {:#?}", expression);
            VariableState::hole()
        },
    }
}

fn evaluate_declared_fn(declared_fn: &DeclaredItemFn<'_>, arguments: Vec<VariableState>) -> VariableState {
    let declaration = &declared_fn.declaration;
    let resolved_arguments: HashMap<_, _> = declaration.args.args.iter().zip(arguments.iter()).map(|(arg, state)| {
        [(arg.arg_binding, state), (arg.arg_name, state)]
    }).flatten().collect();
    let returns = match declaration.returns {
        Some(ref returns) => returns,
        None => {
            // If there are no returns, then we can't know anything about the return value
            return VariableState::hole();
        }
    };

    let mut return_states = VariableState { constraints: vec![] };
    evaluate_expr(&returns, &resolved_arguments, &mut return_states);
    return_states
}

fn evaluate_expr(expr: &Expr, arguments: &HashMap<&str, &VariableState>, variable_state: &mut VariableState) {
    match expr {
        Expr::LitStr(s) => {variable_state.constraints.push(VariableStateConstraint::Value(s.value.to_string()));},
        Expr::Var(v) => {
            let var_states = arguments.get(v.name).unwrap();
            variable_state.constraints.extend(var_states.constraints.clone());
        }
        Expr::Add(add) => {
            evaluate_expr(&add.lhs, arguments, variable_state);
            evaluate_expr(&add.rhs, arguments, variable_state);
        }
    }
}

struct FunctionCall<'a> {
    name: &'a syn::Ident,
    arguments: Vec<&'a syn::Expr>,
}

fn find_variable_metadata<'a>(
    find_var_name: &'a syn::Ident,
    last_before: u16,
    variables: &'a BTreeMap<(u16, &'a syn::Ident), VariableMetadata<'a>>
) -> Option<&'a VariableMetadata<'a>> {
    for ((var_id, var_name), var) in variables.iter().rev() {
        if find_var_name == *var_name && *var_id <= last_before {
            return Some(var);
        }
    }
    None
}

// extract the identifiers from the let binding
fn extract_variables_from_pat(pat: &syn::Pat) -> Vec<&syn::Ident> {
    match pat {
        syn::Pat::Ident(ref ident) => vec![&ident.ident],
        syn::Pat::Tuple(ref tuple) => tuple
            .elems
            .iter()
            .flat_map(|pat| extract_variables_from_pat(&pat))
            .collect(),
        syn::Pat::Wild(_) => vec![],
        _ => panic!("unsupported pattern: {:#?}", pat),
    }
}

#[derive(Debug, Clone)]
pub enum VariableStateConstraint {
    Hole,
    Value(String),
}

#[derive(Debug, Clone)]
pub struct VariableState {
    pub constraints: Vec<VariableStateConstraint>,
}

impl VariableState {
    pub fn optimize(self) -> Self {
        let mut constraints = Vec::with_capacity(1);
        let mut combined = String::new();
        for constraint in self.constraints {
            match constraint {
                VariableStateConstraint::Value(s) => {
                    combined.push_str(&s);
                }
                VariableStateConstraint::Hole => {
                    if combined.is_empty() {
                        constraints.push(VariableStateConstraint::Hole);
                    } else {
                        let combined = std::mem::take(&mut combined);
                        constraints.push(VariableStateConstraint::Value(combined));
                        constraints.push(VariableStateConstraint::Hole);
                    }
                }
            }
        }
        if !combined.is_empty() {
            constraints.push(VariableStateConstraint::Value(combined));
        }
        Self { constraints }
    }

    pub fn value(value: String) -> Self {
        Self {
            constraints: vec![VariableStateConstraint::Value(value)],
        }
    }

    pub fn hole() -> Self {
        Self {
            constraints: vec![VariableStateConstraint::Hole],
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariableMetadata<'a> {
    /// The ident of the variable ie the `x` in `let x = 1;`
    variable_name: &'a syn::Ident,
    /// The instance number of this variable, allowing us to differentiate between
    /// variables with the same name (due to shadowing)
    variable_instance_id: u16,
    /// The `local` statement that declared this variable
    local: &'a syn::Local,
    /// The known constraints on this variable
    variable_state: VariableState,
}

impl<'a> VariableMetadata<'a> {
    pub fn new(
        variable_name: &'a syn::Ident,
        variable_instance_id: u16,
        local: &'a syn::Local,
        variable_state: VariableState,
    ) -> Self {
        Self {
            variable_name,
            variable_instance_id,
            local,
            variable_state,
        }
    }
}