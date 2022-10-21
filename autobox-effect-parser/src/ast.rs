#![allow(unused_imports)]

use nom::branch::alt;
use nom::bytes::complete::{
    tag, take, take_till, take_till1, take_until, take_until1, take_while, take_while1,
};
use nom::character::complete::{alpha1, alphanumeric1, multispace0};
use nom::combinator::{map_res, not, opt, recognize};
use nom::multi::{many0, many0_count, separated_list0};
use nom::sequence::{delimited, pair, preceded};
use nom::{error::ParseError, sequence::separated_pair, IResult};

pub fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}

// Remove whitespace from the beginning and end of a string
fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

#[derive(Debug, Clone)]
pub struct Arg<'a> {
    pub arg_name: &'a str,
    pub arg_binding: &'a str,
}

impl<'a> Arg<'a> {
    fn parse(input: &'a str) -> IResult<&str, Self> {
        let (input, arg_name) = ws(identifier)(input)?;
        let (input, arg_binding) = preceded(ws(tag("as")), identifier)(input)?;

        Ok((
            input,
            Self {
                arg_name,
                arg_binding,
            },
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Args<'a> {
    pub args: Vec<Arg<'a>>,
}

impl<'a> Args<'a> {
    fn parse(input: &'a str) -> IResult<&str, Self> {
        let (input, args) = delimited(
            ws(tag("(")),
            separated_list0(ws(tag(",")), Arg::parse),
            ws(tag(")")),
        )(input)?;
        Ok((input, Self { args }))
    }
}

#[derive(Debug, Clone)]
pub struct Add<'a> {
    pub lhs: Expr<'a>,
    pub rhs: Expr<'a>,
}

impl<'a> Add<'a> {
    fn parse(input: &'a str) -> IResult<&str, Self> {
        println!("Add::parse({:?})", input);
        let (input, lhs) = ws(take_till1(|s| s == '+' || s == ')'))(input)?;
        let _ = take(1usize)(input)?.1;

        let (_, lhs) = ws(Expr::parse)(lhs)?;
        let (input, _) = take(1usize)(input)?;
        let (_, rhs) = ws(Expr::parse)(input)?;
        println!("Add::parse({:?}) => {:?}", input, rhs);
        Ok((input, Self { lhs, rhs }))
    }
}

#[derive(Debug, Clone)]
pub struct LitStr<'a> {
    pub value: &'a str,
}

impl<'a> LitStr<'a> {
    fn parse(input: &'a str) -> IResult<&str, Self> {
        println!("litstr input = {:?}", input);
        let (input, value) = alt((
            delimited(tag("'"), take_till1(|c| c == '\''), tag("'")),
            delimited(tag("\""), take_till1(|c| c == '"'), tag("\"")),
        ))(input)?;
        println!("litstr value = {:?}", value);
        Ok((input, Self { value }))
    }
}

#[derive(Debug, Clone)]
pub struct Var<'a> {
    pub name: &'a str,
}

impl<'a> Var<'a> {
    fn parse(input: &'a str) -> IResult<&str, Self> {
        println!("var input = {:?}", input);
        let (input, name) = identifier(input)?;
        println!("var name = {:?}", name);
        Ok((input, Self { name }))
    }
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    LitStr(LitStr<'a>),
    Var(Var<'a>),
    Add(Box<Add<'a>>),
}

impl<'a> Expr<'a> {
    #[track_caller]
    pub fn unwrap_lit_str(&self) -> &LitStr<'a> {
        match self {
            Expr::LitStr(lit_str) => lit_str,
            _ => panic!("Expected LitStr"),
        }
    }

    #[track_caller]
    pub fn unwrap_var(&self) -> &Var {
        match self {
            Expr::Var(var) => var,
            _ => panic!("Expected Var"),
        }
    }

    #[track_caller]
    pub fn unwrap_add(&self) -> &Add<'a> {
        match self {
            Expr::Add(add) => add,
            _ => panic!("Expected Add"),
        }
    }
}

impl<'a> Expr<'a> {
    pub fn parse(input: &'a str) -> IResult<&str, Self> {
        println!("Parsing expr: {}", input);
        let (input, expr) = alt((
            map_res(ws(Add::parse), |add| {
                Ok::<Expr<'_>, &str>(Expr::Add(Box::new(add)))
            }),
            map_res(ws(LitStr::parse), |s| Ok::<Expr<'_>, &str>(Expr::LitStr(s))),
            map_res(ws(Var::parse), |var| Ok::<Expr<'_>, &str>(Expr::Var(var))),
        ))(input)?;

        println!("Parsed expr: {:?}", expr);

        Ok((input, expr))
    }
}

#[derive(Debug, Clone)]
pub struct SideEffectStmt<'a> {
    pub side_effect_name: &'a str,
    pub side_effect_arguments: Vec<Expr<'a>>,
    pub binding: Option<&'a str>,
}

impl<'a> SideEffectStmt<'a> {
    pub fn parse(input: &'a str) -> IResult<&str, Self> {
        let (input, side_effect_name) = ws(identifier)(input)?;
        let (input, args) = delimited(ws(tag("(")), take_till1(|c| c == ')'), ws(tag(")")))(input)?;
        println!("side effect args = {:?}", args);
        println!("side effect input = {:?}", input);
        let (_input, side_effect_arguments) = separated_list0(ws(tag(",")), ws(Expr::parse))(args)?;

        println!(
            "side effect side_effect_arguments = {:?}",
            side_effect_arguments
        );
        println!("side effect input = {:?}", input);
        let (input, binding) = opt(preceded(ws(tag("as")), identifier))(input)?;
        Ok((
            input,
            Self {
                side_effect_name,
                side_effect_arguments,
                binding,
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct SideEffects<'a> {
    pub side_effect_stmts: Vec<SideEffectStmt<'a>>,
}

impl<'a> SideEffects<'a> {
    pub fn parse(input: &'a str) -> IResult<&str, Self> {
        let (input, _) = ws(tag("("))(input)?;
        let (input, side_effect_stmts) =
            separated_list0(ws(tag(",")), SideEffectStmt::parse)(input)?;
        let (input, _) = ws(tag(")"))(input)?;
        let (input, _) = opt(ws(tag(",")))(input)?;
        Ok((input, Self { side_effect_stmts }))
    }
}

#[derive(Debug, Clone)]
pub struct DeclareMacro<'a> {
    pub args: Args<'a>,
    pub side_effects: SideEffects<'a>,
    pub returns: Option<Expr<'a>>,
}

impl<'a> DeclareMacro<'a> {
    pub fn parse(input: &'a str) -> IResult<&str, Self> {
        let (input, args) = opt(delimited(ws(tag("args=")), Args::parse, ws(tag(","))))(input)?;
        let (input, side_effects) = preceded(ws(tag("side_effects=")), SideEffects::parse)(input)?;

        let (input, returns) = opt(preceded(ws(tag("returns=")), Expr::parse))(input)?;

        Ok((
            input,
            Self {
                args: args.unwrap_or_default(),
                side_effects,
                returns,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_parse() {
        let (rest, arg) = Arg::parse("foo as bar").unwrap();
        assert_eq!(arg.arg_name, "foo");
        assert_eq!(arg.arg_binding, "bar");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_args_parse() {
        let (rest, args) = Args::parse("(foo as bar, baz as qux)").unwrap();
        assert_eq!(args.args.len(), 2);
        assert_eq!(args.args[0].arg_name, "foo");
        assert_eq!(args.args[0].arg_binding, "bar");
        assert_eq!(args.args[1].arg_name, "baz");
        assert_eq!(args.args[1].arg_binding, "qux");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_side_effect_stmt() {
        let (rest, side_effect_stmt) = SideEffectStmt::parse("read_file(bar, baz) as qux").unwrap();
        assert_eq!(side_effect_stmt.side_effect_name, "read_file");
        assert_eq!(side_effect_stmt.side_effect_arguments.len(), 2);
        assert_eq!(
            side_effect_stmt.side_effect_arguments[0].unwrap_var().name,
            "bar"
        );
        assert_eq!(
            side_effect_stmt.side_effect_arguments[1].unwrap_var().name,
            "baz"
        );
        assert_eq!(side_effect_stmt.binding, Some("qux"));
        assert_eq!(rest, "");
    }

    #[test]
    fn test_side_effects_parse() {
        let (rest, side_effects) = SideEffects::parse(
            "\
            (\
            eval(T + '/') as U,\
            eval(T),\
            read_file(bar, baz) as qux\
            )\
        ",
        )
        .unwrap();
        assert_eq!(rest, "");
        assert_eq!(side_effects.side_effect_stmts.len(), 3);
        let side_effect_stmt = &side_effects.side_effect_stmts[0];
        assert_eq!(side_effect_stmt.side_effect_name, "eval");
        assert_eq!(side_effect_stmt.side_effect_arguments.len(), 1);
        assert_eq!(
            side_effect_stmt.side_effect_arguments[0]
                .unwrap_add()
                .lhs
                .unwrap_var()
                .name,
            "T"
        );
        assert_eq!(
            side_effect_stmt.side_effect_arguments[0]
                .unwrap_add()
                .rhs
                .unwrap_lit_str()
                .value,
            "/"
        );
        assert_eq!(side_effect_stmt.binding, Some("U"));

        let side_effect_stmt = &side_effects.side_effect_stmts[1];
        assert_eq!(side_effect_stmt.side_effect_name, "eval");
        assert_eq!(side_effect_stmt.side_effect_arguments.len(), 1);
        assert_eq!(
            side_effect_stmt.side_effect_arguments[0].unwrap_var().name,
            "T"
        );
        assert_eq!(side_effect_stmt.binding, None);

        let side_effect_stmt = &side_effects.side_effect_stmts[2];
        assert_eq!(side_effect_stmt.side_effect_name, "read_file");
        assert_eq!(side_effect_stmt.side_effect_arguments.len(), 2);
        assert_eq!(
            side_effect_stmt.side_effect_arguments[0].unwrap_var().name,
            "bar"
        );
        assert_eq!(
            side_effect_stmt.side_effect_arguments[1].unwrap_var().name,
            "baz"
        );
        assert_eq!(side_effect_stmt.binding, Some("qux"));
    }

    #[test]
    #[should_panic] // todo: Nested expressions are not supported yet
    fn test_expr_nested_parens() {
        let (rest, expr) = Expr::parse("((T + '/') + U)").unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            expr.unwrap_add().lhs.unwrap_add().lhs.unwrap_var().name,
            "T"
        );
        assert_eq!(
            expr.unwrap_add()
                .lhs
                .unwrap_add()
                .rhs
                .unwrap_lit_str()
                .value,
            "/"
        );
        assert_eq!(expr.unwrap_add().rhs.unwrap_lit_str().value, "U");
    }

    #[test]
    #[should_panic] // todo: Chained expressions are not supported yet
    fn test_expr_chain() {
        let (rest, expr) = Expr::parse("(T + '/' + U").unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            expr.unwrap_add().lhs.unwrap_add().lhs.unwrap_var().name,
            "T"
        );
        assert_eq!(
            expr.unwrap_add()
                .lhs
                .unwrap_add()
                .rhs
                .unwrap_lit_str()
                .value,
            "/"
        );
        assert_eq!(expr.unwrap_add().rhs.unwrap_lit_str().value, "U");
    }

    #[test]
    fn test_declare_macro_parse() {
        let declare_macro = r"
            args=(foo as F, baz as F),
            side_effects=(
                eval(F + '/') as FS,
                eval(FS + B) as result,
                read_file(result)
            )
        ";
        let (rest, declare_macro) = DeclareMacro::parse(declare_macro).unwrap();
        assert_eq!(rest, "");
        assert_eq!(declare_macro.args.args.len(), 2);
        assert_eq!(declare_macro.args.args[0].arg_name, "foo");
        assert_eq!(declare_macro.args.args[0].arg_binding, "F");
        assert_eq!(declare_macro.args.args[1].arg_name, "baz");
        assert_eq!(declare_macro.args.args[1].arg_binding, "F");
        assert_eq!(declare_macro.side_effects.side_effect_stmts.len(), 3);
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[0].side_effect_name,
            "eval"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[0]
                .side_effect_arguments
                .len(),
            1
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[0].side_effect_arguments[0]
                .unwrap_add()
                .lhs
                .unwrap_var()
                .name,
            "F"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[0].side_effect_arguments[0]
                .unwrap_add()
                .rhs
                .unwrap_lit_str()
                .value,
            "/"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[0].binding,
            Some("FS")
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[1].side_effect_name,
            "eval"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[1]
                .side_effect_arguments
                .len(),
            1
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[1].side_effect_arguments[0]
                .unwrap_add()
                .lhs
                .unwrap_var()
                .name,
            "FS"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[1].side_effect_arguments[0]
                .unwrap_add()
                .rhs
                .unwrap_var()
                .name,
            "B"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[1].binding,
            Some("result")
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[2].side_effect_name,
            "read_file"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[2]
                .side_effect_arguments
                .len(),
            1
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[2].side_effect_arguments[0]
                .unwrap_var()
                .name,
            "result"
        );
        assert_eq!(
            declare_macro.side_effects.side_effect_stmts[2].binding,
            None
        );
    }

    #[test]
    fn test_expr_lit_str_parse() {
        let (rest, expr) = Expr::parse(r#""foo""#).unwrap();
        let lit_str = expr.unwrap_lit_str();
        assert_eq!(lit_str.value, "foo");
    }

    #[test]
    fn test_expr_var_parse() {
        let (rest, expr) = Expr::parse("foo").unwrap();
        let var = expr.unwrap_var();
        assert_eq!(var.name, "foo");
    }

    #[test]
    fn test_expr_add_vars_parse() {
        let (rest, expr) = Expr::parse("foo + bar").unwrap();
        let add_op = expr.unwrap_add();
        assert_eq!(add_op.lhs.unwrap_var().name, "foo");
        assert_eq!(add_op.rhs.unwrap_var().name, "bar");
    }

    #[test]
    fn test_expr_add_lit_var_parse() {
        let (rest, expr) = Expr::parse("'foo' + bar").unwrap();
        println!("{:?}", expr);
        let add_op = expr.unwrap_add();
        assert_eq!(add_op.lhs.unwrap_lit_str().value, "foo");
        assert_eq!(add_op.rhs.unwrap_var().name, "bar");
    }

    #[test]
    fn test_expr_add_nested_parse() {
        let (rest, expr) = Expr::parse("'foo' + bar + baz").unwrap();
        println!("{:?}", expr);
        let add_op = expr.unwrap_add();
        assert_eq!(add_op.lhs.unwrap_lit_str().value, "foo");
        let rhs = add_op.rhs.unwrap_add();
        assert_eq!(rhs.lhs.unwrap_var().name, "bar");
        assert_eq!(rhs.rhs.unwrap_var().name, "baz");
    }
}