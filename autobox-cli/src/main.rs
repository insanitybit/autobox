#![allow(unused_imports, dead_code, unreachable_code)]

use eyre::{eyre, Report, Result, WrapErr};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use syn::__private::ToTokens;
use syn::visit::{self, Visit};
use syn::{Attribute, ExprCall, ExprPath, ItemFn};
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

// example: T + "/" + U as O
#[derive(Clone, Debug)]
struct EffectExpr {
    // todo: Evaluate tokens into simple AST
    tokens: String,
}

impl EffectExpr {
    fn execute(&self, args: &HashMap<String, String>) -> String {
        let result = self.tokens.clone();
        // remove single quotes, which are literals
        let mut result = result.replace("'", "");
        // This is a hack that *will* break things trivially
        // example: if it replaces "T" with "value" and then
        // one of the args is named "value" it will replace *that* too
        // - awful, moving on
        for (key, value) in args {
            if key == "_" {
                continue;
            }
            result = result.replace(key, value);
        }
        // We only support "+" for now lol sorry
        let parts = result.split(" + ");
        parts.collect()
    }
}

// example:
// read_file(T)
//           ^
#[derive(Clone, Debug)]
enum SideEffectArg {
    // todo: support expr, not doing that today, use `eval`
    // example: `read_file(T + "/" + U)`
    //                     ^^^^^^^^^^^
    Expr(EffectExpr),
    // example: `read_file(T)`
    //                     ^
    Binding(String),
}

impl SideEffectArg {
    fn as_binding(&self) -> String {
        match self {
            SideEffectArg::Expr(_) => {
                panic!("Not a binding")
            }
            SideEffectArg::Binding(b) => b.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedSideEffect {
    name: String,
    args: Vec<String>,
}

/// example: `read_file(U) as O`
#[derive(Clone, Debug)]
struct SideEffectClause {
    /// example: `read_file`
    effect_name: String,
    /// Comma separated list of arguments to the side effect
    /// example: `U`
    effect_args: Vec<SideEffectArg>,
    /// example: `O`
    binding: String, // "" if no binding
}

impl SideEffectClause {
    fn execute(&self, resolved_args: &HashMap<String, String>) -> ResolvedSideEffect {
        // todo: support binding the output of the side effect
        let mut args = Vec::new();
        for effect_arg in &self.effect_args {
            let arg_binding = effect_arg.as_binding(); // panic on expr
            match resolved_args.get(&arg_binding) {
                Some(value) => {
                    args.push(value.clone());
                }
                None => {
                    panic!("No value for binding {}", arg_binding);
                }
            }
        }
        ResolvedSideEffect {
            name: self.effect_name.clone(),
            args,
        }
    }
}

/// note: Expressions start with `eval`
/// note: All Expressions must come before side effects
/// example:
/// side_effects=(
///    eval(F + "/" + T) as U,
///    read_file(F + "/" + T) as O
/// )
#[derive(Default, Debug, Clone)]
struct SideEffects {
    /// side_effects=(
    ///    eval(F + "/" + T) as U,
    /// // ^^^^^^^^^^^^^^^^^ expressions start with `eval` (dumb but idk)
    ///    read_file(U) as O
    /// )
    expressions: Vec<DeclareExpr>,
    /// side_effects=(
    ///    eval(F + "/" + T) as U,
    ///    read_file(U) as O
    /// // ^^^^^^^^^^^^^^^^^ effect clauses
    /// )
    effect_clauses: Vec<SideEffectClause>,
}

impl SideEffects {
    // resolve any new bindings created in the `eval` expressions
    // add those to `resolved_args`
    fn execute(&self, resolved_args: &mut HashMap<String, String>) -> Vec<ResolvedSideEffect> {
        for expr in &self.expressions {
            expr.execute(resolved_args);
        }

        // todo: Not supporting tracking the values returned by an effect function rn
        let mut resolved_effects = Vec::new();
        for effect_clause in &self.effect_clauses {
            let resolved_effect = effect_clause.execute(resolved_args);
            resolved_effects.push(resolved_effect);
        }

        resolved_effects
    }
}

fn find_closing_paren(s: &str) -> usize {
    let mut paren_count = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => paren_count += 1,
            ')' => {
                paren_count -= 1;
                if paren_count == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    panic!("No closing paren")
}

// fixme: This parser sucks and relies on whitespace and formatting quirks of a Display impl
fn parse_side_effects(tokens: &str) -> Option<SideEffects> {
    if tokens.find("side_effects =").is_none() {
        return None;
    }

    let side_effects_start = tokens.find("side_effects = (").unwrap() + "side_effects = ".len();
    let side_effects_end = find_closing_paren(&tokens[side_effects_start..]);
    let tokens = &tokens[side_effects_start + 1..][..side_effects_end];
    let mut expressions = Vec::new();
    let mut effect_clauses = Vec::new();

    let mut expression = tokens;

    loop {
        if !expression.starts_with("eval") {
            break;
        }
        if expression.is_empty() {
            break;
        }
        let line_ix = expression.find(',');
        let (line, line_ix) = match line_ix {
            // skip "eval"
            Some(line_ix) => (&expression[4..line_ix], line_ix),
            None => break,
        };

        let mut split = line.split(" as ");
        let expr = split.next().unwrap().trim();
        let expr = remove_surrounding(expr, "(");
        let binding = split.next().unwrap().trim();
        expressions.push(DeclareExpr {
            expression: EffectExpr {
                tokens: expr.to_string(),
            },
            binding: binding.to_string(),
        });

        expression = &expression[line_ix + 1..]
    }

    loop {
        if expression.is_empty() {
            break;
        }
        let line_ix = expression.find(',');
        let (line, line_ix) = match line_ix {
            Some(line_ix) => (&expression[4..line_ix], line_ix),
            None => (&expression[..], expression.len()),
        };
        let mut split = line.split('(');
        let fn_name = split.next().unwrap().trim();
        let mut split = split.next().unwrap().split(") as ");
        let args = split.next().unwrap().trim();
        let binding = split.next().unwrap().trim();
        let binding = binding.strip_suffix(")").unwrap_or(binding);

        effect_clauses.push(SideEffectClause {
            effect_name: fn_name.to_string(),
            effect_args: args
                .split(", ")
                .map(|arg| {
                    if arg.starts_with("eval") {
                        SideEffectArg::Expr(EffectExpr {
                            tokens: arg.to_string(),
                        })
                    } else {
                        SideEffectArg::Binding(arg.to_string())
                    }
                })
                .collect(),
            binding: binding.to_string(),
        });
        expression = &expression[line_ix..]
    }
    Some(SideEffects {
        expressions,
        effect_clauses,
    })
}

/// example: `foo as F`
#[derive(Debug, Clone)]
struct DeclareArg {
    /// `foo`
    argument_name: String,
    /// `F`
    argument_binding: String,
}

impl DeclareArg {
    fn parse_arg(input: &str) -> Self {
        let mut pair = input.split(" as ");
        let argument_name = pair.next().unwrap().to_string();
        let argument_binding = pair.next().unwrap().to_string();
        Self {
            argument_name,
            argument_binding,
        }
    }
}

/// An EffectExpr in a `declare` invocation
#[derive(Clone, Debug)]
struct DeclareExpr {
    expression: EffectExpr,
    binding: String,
}

impl DeclareExpr {
    fn execute(&self, resolved_args: &mut HashMap<String, String>) {
        let value = self.expression.execute(resolved_args);
        resolved_args.insert(self.binding.clone(), value.clone());
    }
}

/// The `declare` macro attribute
#[derive(Clone, Debug)]
struct DeclareAttribute {
    args: Vec<DeclareArg>,
    side_effects: Option<SideEffects>,
    returns: EffectExpr,
}

fn remove_surrounding<'a>(s: &'a str, r: &str) -> &'a str {
    let s = s.strip_prefix(r).unwrap_or(s);
    let r = match r {
        "(" => ")",
        r => r,
    };
    s.strip_suffix(r).unwrap_or(s)
}

impl DeclareAttribute {
    fn execute(&self, parameters: Vec<String>) -> Vec<ResolvedSideEffect> {
        let mut resolved_args = self
            .args
            .iter()
            .zip(&parameters)
            .map(|(arg, param)| {
                (
                    arg.argument_name.clone(),
                    remove_surrounding(param, "\"").to_string(),
                )
            })
            .collect::<HashMap<_, _>>();
        for (arg, param) in self.args.iter().zip(parameters) {
            resolved_args.insert(
                arg.argument_binding.clone(),
                remove_surrounding(&param, "\"").to_string(),
            );
        }

        // execute side effects
        if let Some(ref side_effects) = self.side_effects {
            side_effects.execute(&mut resolved_args)
        } else {
            vec![]
        }
    }
}

// This "parser" sucks and relies on the token formatting being
// precise
fn parse_args(args: &str) -> Vec<DeclareArg> {
    let start_index = args.find("args = (").unwrap() + "args = (".len();
    let end_index = args[start_index..].find(")").unwrap();
    let args = &args[start_index..][..end_index];
    let mut parsed_args = Vec::new();
    let args = args.split(",");
    for arg in args {
        let arg = arg.trim();
        let arg = DeclareArg::parse_arg(arg);
        parsed_args.push(arg);
    }
    parsed_args
}

fn parse_returns(returns: &str) -> EffectExpr {
    let start_index = returns.find("returns = (").unwrap() + "returns = ( ".len();
    let end_index = returns[start_index..].find(")").unwrap();
    let returns = &returns[start_index..][..end_index];
    EffectExpr {
        tokens: returns.to_string(),
    }
}

impl DeclareAttribute {
    fn from_attribute(attribute: &Attribute) -> Self {
        let attribute_tokens = attribute.tokens.to_string();
        let args = parse_args(&attribute_tokens);
        let side_effects = parse_side_effects(&attribute_tokens);
        let returns = parse_returns(&attribute_tokens);
        Self {
            args,
            side_effects,
            returns,
        }
    }
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

    let mut declarations = HashMap::new();

    for name in declared_fn_calls.keys() {
        // todo: handle more than one attribute
        let attribute = &declared_item_fns[&name[..]].attrs[0];
        let declaration = DeclareAttribute::from_attribute(attribute);
        declarations.insert(name.clone(), declaration);
    }

    let mut resolved_side_effects = Vec::new();
    // Find the calls, evaluate them given their arguments
    for (name, call) in declared_fn_calls {
        // todo: for now we don't do any fancy resolution of values, we just
        // assume they are static
        // retrieve the arguments to the call

        let arguments = resolve_arguments(call.clone());
        let declaration = declarations.get(&name[..]).unwrap();
        resolved_side_effects.extend_from_slice(&declaration.execute(arguments)[..]);
    }

    println!("resolved_side_effects: {:?}", resolved_side_effects);

    // and now we can generate policy

    Ok(())
}
