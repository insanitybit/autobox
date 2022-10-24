// #![allow(unused_imports, dead_code, unreachable_code, unused_variables)]

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use eyre::Result;
use syn::{ExprCall, Ident, ItemFn, Stmt};
use syn::visit::{self, Visit};

use autobox_effect_parser::ast::{Arg, DeclareMacro, Expr};

fn read_ast(path: &Path) -> Result<syn::File> {
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
    item_fns: Vec<&'ast ItemFn>,
}

impl<'ast> ItemFnVisitor<'ast> {
    fn collect_from_ast(ast: &'ast syn::File) -> Self {
        let mut visitor = ItemFnVisitor::default();
        visitor.visit_file(ast);
        visitor
    }
}

impl<'ast> Visit<'ast> for ItemFnVisitor<'ast> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.item_fns.push(node);
        visit::visit_item_fn(self, node);
    }
}

#[derive(Clone, Debug, Default)]
struct FnCallVisitor<'ast> {
    fn_calls: Vec<&'ast ExprCall>,
}

impl<'ast> Visit<'ast> for FnCallVisitor<'ast> {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        self.fn_calls.push(node);
        visit::visit_expr_call(self, node);
    }
}

fn main() -> Result<()> {
    println!(
        "running from: {}",
        std::env::current_dir().unwrap().display()
    );

    // for entry in WalkDir::new("./example-app/src/") {

    // First we find the entrypoint, then we find all function calls,
    let ast = read_ast(Path::new("./example-app/src/main.rs"))?;
    let entrypoint = find_entrypoint(&ast).unwrap();
    let all_fn_items: HashMap<_, _> = ItemFnVisitor::collect_from_ast(&ast).item_fns.into_iter()
        .map(|f| (&f.sig.ident, f))
        .collect();
    let all_declared_fns = get_all_declared_fns(&ast);

    // The inferred declaration of the entrypoint
    let fn_arguments = Vec::new(); // no arguments to entrypoint
    let mut side_effects = Vec::new();
    let _ = infer_fn(&mut side_effects, &entrypoint, &fn_arguments, &all_declared_fns, &all_fn_items);

    for side_effect in side_effects {
        println!("Side effect: {}", side_effect);
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_var() {
        let rust_code = r#"

        // declare_ext!(std::path::Path::join, args(a, b), returns(a + b))

        #[effect::declare(
            args=(a as A, b as B),
            side_effects=(reads_file(A + '/' + B)),
            returns=(a + '/' + b),
        )]
        fn fn_with_effects(a: &str, b: &str) -> String {
            // pretend there are effects here
            std::fs::read_file(format!("{a}/{b}"));
            format!("{a}/{b}")
        }

        fn unknown(a: &str, b: &str) -> String {
            fn_with_effects(a, b)
        }

        #[effect::entrypoint]
        fn main() {
            let x = "foo";
            let y = x;
            let uk = unknown(x, "bar");
        }
        "#;

        let ast = syn::parse_file(rust_code).unwrap();
        let entrypoint = find_entrypoint(&ast).unwrap();
        let all_fn_items: HashMap<_, _> = ItemFnVisitor::collect_from_ast(&ast).item_fns.into_iter()
            .map(|f| (&f.sig.ident, f))
            .collect();
        let all_declared_fns = get_all_declared_fns(&ast);

        // The inferred declaration of the entrypoint
        let fn_arguments = Vec::new(); // no arguments to entrypoint
        let mut side_effects = Vec::new();
        let _entrypoint_declaration = infer_fn(&mut side_effects, &entrypoint, &fn_arguments, &all_declared_fns, &all_fn_items);

    }
}

#[derive(Debug, Clone)]
struct SideEffect {
    name: String,
    arguments: Vec<VariableState>,
}

impl Display for SideEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.arguments.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", globhole(arg))?;
        }
        write!(f, ")")
    }
}

// Given a function that does not been marked with `declare`, infer what the DeclareMacro output
// would have looked like by inspecting its body
fn infer_fn<'a>(
    side_effects: &mut Vec<SideEffect>,
    item_fn: &'a ItemFn,
    fn_arguments: &Vec<VariableState>,
    all_declared_fns: &HashMap<String, DeclaredItemFn<'a>>,
    all_item_fns: &HashMap<&'a Ident, &'a ItemFn>,
) -> VariableState {
    let mut variables = BTreeMap::new();
    let mut args = Vec::with_capacity(item_fn.sig.inputs.len());
    for (arg_i, fn_arg) in item_fn.sig.inputs.iter().enumerate() {
        let arg_name = match fn_arg {
            syn::FnArg::Typed(arg) => match &*arg.pat {
                syn::Pat::Ident(ident) => &ident.ident,
                _ => panic!("unexpected pattern"),
            },
            _ => panic!("unexpected function argument"),
        };
        args.push(Arg {
            arg_name: Cow::Owned(arg_name.to_string()),
            arg_binding: "",
        });
        variables.insert((0, arg_name), VariableMetadata::new(
            Some(arg_name),
            0,
            fn_arguments[arg_i].clone()
        ));
    }

    for (i, statement) in item_fn.block.stmts.iter().enumerate() {
        let i = i as u16;  // dont put > 2^16 statements in your code!!!

        match statement {
            Stmt::Local(local) => {
                // for now we only support single Ident bindings ie: `let x` but not `let Some(x)`
                let var_name = extract_variables_from_pat(&local.pat)[0];
                variables.insert(
                    (i, var_name),
                    VariableMetadata::new(
                        var_name,
                        i as u16,
                        get_variable_state(
                            &local.init.as_ref().expect("variables must be initialized at declaration").1.as_ref(),
                            side_effects,
                            i,
                            &variables,
                            all_declared_fns,
                            &all_item_fns
                        ),
                    )
                );
            }
            Stmt::Semi(syn::Expr::Call(_fn_call), _) => {
            }
            _st => {
                // println!("\n{st:?}\n")
            }
        }
    }

    let (i, statement) = item_fn.block.stmts.iter().enumerate().last().unwrap();
    let returns = match statement {
        Stmt::Expr(expr) => {
            Some(get_variable_state(
                &expr,
                side_effects,
                i as u16,
                &variables,
                all_declared_fns,
                &all_item_fns
            ))
        }
        _ => None,
    };
    returns.unwrap_or(VariableState::hole())
}

#[derive(Debug, Clone)]
struct DeclaredItemFn<'a> {
    declaration: DeclareMacro<'a>,
}

/// find all functions that are marked `declare`
fn get_all_declared_fns(ast: &syn::File) -> HashMap<String, DeclaredItemFn> {
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
    side_effects: &mut Vec<SideEffect>,
    index: u16,
    variables: &BTreeMap<(u16, &syn::Ident), VariableMetadata>,
    all_declared_fns: &HashMap<String, DeclaredItemFn<'_>>,
    all_item_fns: &HashMap<&syn::Ident, &ItemFn>,
) -> VariableState {
    // println!("tokens: {tokens}");
    match expression {
        // x in `let y = x;`
        syn::Expr::Path(ref path) => {
            // println!("PATH: {expression:?}");
            let var_name = &path.path.segments[0].ident;
            let var = find_variable_metadata(var_name, index, variables).unwrap();
            var.variable_state.clone()
        }
        // "foo" in `let y = "foo";`
        syn::Expr::Lit(ref lit) => {
            // println!("LIT: {expression:?}");
            match lit.lit {
                syn::Lit::Str(ref s) => VariableState::value(s.value()),
                _ => {
                    VariableState::hole()
                },
            }
        }
        // foo("bar") in `let y = foo("bar");`
        syn::Expr::Call(ref call) => {
            let fn_name = match call.func.as_ref() {
                syn::Expr::Path(ref path) => &path.path.segments[0].ident,
                _ => {
                    panic!("unsupported function call: {:#?}", expression);
                }
            };

            // println!("Evaluating fn_name: {fn_name}");

            // Given a call we need to calculate the return value
            // based on its inputs
            let mut arg_states = Vec::new();
            for arg in call.args.iter() {
                arg_states.push(get_variable_state(arg, side_effects,index, variables, all_declared_fns, all_item_fns));
            }

            match all_declared_fns.get(&fn_name.to_string()) {
                Some(f) => {
                    evaluate_declared_fn(side_effects, f, arg_states)
                },
                None => {
                    // We must infer this function's declaration
                    infer_fn(side_effects, &all_item_fns[fn_name], &arg_states, all_declared_fns, all_item_fns)
                }
            }
            // Now we have to calculate the state of the return value
        }
        syn::Expr::Reference(
            syn::ExprReference {expr, ..}
        ) => {
            get_variable_state(expr.as_ref(), side_effects, index, variables, all_declared_fns, all_item_fns)
        }
        _ => {
            eprintln!("Unsupported expression type for: {:#?}", expression);
            VariableState::hole()
        },
    }
}

fn evaluate_declared_fn(
    side_effects: &mut Vec<SideEffect>,
    declared_fn: &DeclaredItemFn<'_>,
    arguments: Vec<VariableState>
) -> VariableState {
    let declaration = &declared_fn.declaration;
    let resolved_arguments: HashMap<_, _> = declaration.args.args.iter().zip(arguments.iter()).map(|(arg, state)| {
        [(arg.arg_binding, state), (arg.arg_name.as_ref(), state)]
    }).flatten().collect();
    let returns = match declaration.returns {
        Some(ref returns) => returns,
        None => {
            // If there are no returns, then we can't know anything about the return value
            return VariableState::hole();
        }
    };

    let declared_side_effect_stmts = declaration.side_effects.clone().unwrap_or_default().side_effect_stmts;
    for effect in declared_side_effect_stmts {
        let mut side_effect = SideEffect {
            name: effect.side_effect_name.to_string(),
            arguments: Vec::new(),
        };
        for arg in effect.side_effect_arguments {
            let mut state = VariableState::empty();
            evaluate_expr(&arg, &resolved_arguments, &mut state);
            side_effect.arguments.push(state);
        }
        side_effects.push(side_effect);
    }

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
enum VariableStateConstraint {
    Hole,
    Value(String),
}


#[derive(Debug, Clone)]
struct VariableState {
    constraints: Vec<VariableStateConstraint>,
}

impl VariableState {
    fn empty() -> Self {
        Self {
            constraints: vec![],
        }
    }

    fn value(value: String) -> Self {
        Self {
            constraints: vec![VariableStateConstraint::Value(value)],
        }
    }

    fn hole() -> Self {
        Self {
            constraints: vec![VariableStateConstraint::Hole],
        }
    }
}

fn globhole(state: &VariableState) -> String {
    let mut globholed = String::with_capacity(state.constraints.len());

    for constraint in state.constraints.iter() {
        match constraint {
            VariableStateConstraint::Hole => globholed.push_str("*"),
            VariableStateConstraint::Value(value) => globholed.push_str(&value),
        }
    }

    globholed
}

#[derive(Debug, Clone)]
struct VariableMetadata<'a> {
    /// The ident of the variable ie the `x` in `let x = 1;`
    #[allow(dead_code)]
    variable_name: Option<&'a syn::Ident>,
    /// The instance number of this variable, allowing us to differentiate between
    /// variables with the same name (due to shadowing)
    #[allow(dead_code)]
    variable_instance_id: u16,
    /// The known constraints on this variable
    variable_state: VariableState,
}

impl<'a> VariableMetadata<'a> {
    fn new(
        variable_name: impl Into<Option<&'a syn::Ident>>,
        variable_instance_id: u16,
        variable_state: VariableState,
    ) -> Self {
        Self {
            variable_name: variable_name.into(),
            variable_instance_id,
            variable_state,
        }
    }
}