//! Independent structured-source oracle: no OIR slots, lowering helpers, or
//! production dispatcher are used to calculate expected values or path traces.
//! Generated call graphs are acyclic; model nesting is deliberately <= 8.
use super::{tests::compiled, *};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Bool(bool),
    Unit,
}
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(x) => write!(f, "{x}"),
            Self::Unit => f.write_str("()"),
        }
    }
}
#[derive(Clone)]
enum Expr {
    Literal(Value),
    Name(&'static str),
    Group(Box<Expr>),
    Call(usize, Vec<Expr>),
}
#[derive(Clone)]
enum Stmt {
    Let(&'static str, bool, Expr),
    Discard(Expr),
    Return(Option<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
}
struct Definition {
    name: String,
    params: Vec<(&'static str, &'static str)>,
    ty: &'static str,
    body: Vec<Stmt>,
}
struct Model {
    functions: Vec<Definition>,
}
impl Model {
    fn expression(
        &self,
        expr: &Expr,
        env: &BTreeMap<&str, Value>,
        trace: &mut Vec<String>,
        depth: usize,
    ) -> Value {
        assert!(depth <= 8);
        match expr {
            Expr::Literal(value) => *value,
            Expr::Name(name) => env[name],
            Expr::Group(inner) => self.expression(inner, env, trace, depth),
            Expr::Call(id, args) => {
                let values = args
                    .iter()
                    .map(|arg| self.expression(arg, env, trace, depth))
                    .collect();
                self.call(*id, values, trace, depth + 1)
            }
        }
    }
    fn block(
        &self,
        body: &[Stmt],
        mut env: BTreeMap<&str, Value>,
        trace: &mut Vec<String>,
        depth: usize,
        name: &str,
    ) -> Option<Value> {
        for stmt in body {
            match stmt {
                Stmt::Let(name, _, expr) => {
                    let value = self.expression(expr, &env, trace, depth);
                    assert!(env.insert(name, value).is_none());
                }
                Stmt::Discard(expr) => {
                    self.expression(expr, &env, trace, depth);
                }
                Stmt::Return(expr) => {
                    return Some(expr.as_ref().map_or(Value::Unit, |expr| {
                        self.expression(expr, &env, trace, depth)
                    }))
                }
                Stmt::If(expr, yes, no) => {
                    let Value::Bool(condition) = self.expression(expr, &env, trace, depth) else {
                        panic!("model condition must be bool")
                    };
                    trace.push(format!("branch {name} {condition}"));
                    if let Some(value) = self.block(
                        if condition { yes } else { no },
                        env.clone(),
                        trace,
                        depth + 1,
                        name,
                    ) {
                        return Some(value);
                    }
                }
            }
        }
        None
    }
    fn call(&self, id: usize, values: Vec<Value>, trace: &mut Vec<String>, depth: usize) -> Value {
        assert!(depth <= 8);
        let f = &self.functions[id];
        assert_eq!(f.params.len(), values.len());
        let env = f.params.iter().map(|(name, _)| *name).zip(values).collect();
        trace.push(format!("enter {}", f.name));
        let value = self
            .block(&f.body, env, trace, depth, &f.name)
            .expect("model complete return");
        trace.push(format!("return {} {value}", f.name));
        value
    }
    fn expression_source(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(value) => value.to_string(),
            Expr::Name(name) => name.to_string(),
            Expr::Group(inner) => format!("({})", self.expression_source(inner)),
            Expr::Call(id, args) => format!(
                "{}({})",
                self.functions[*id].name,
                args.iter()
                    .map(|arg| self.expression_source(arg))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
    fn block_source(&self, body: &[Stmt]) -> String {
        body.iter()
            .map(|stmt| match stmt {
                Stmt::Let(name, annotated, expr) => format!(
                    "let {name}{} = {};",
                    if *annotated { ": bool" } else { "" },
                    self.expression_source(expr)
                ),
                Stmt::Discard(expr) => format!("{};", self.expression_source(expr)),
                Stmt::Return(expr) => format!(
                    "return {};",
                    expr.as_ref()
                        .map_or(String::new(), |expr| self.expression_source(expr))
                ),
                Stmt::If(condition, yes, no) => format!(
                    "if {} {{ {} }}{}",
                    self.expression_source(condition),
                    self.block_source(yes),
                    if no.is_empty() {
                        String::new()
                    } else {
                        format!(" else {{ {} }}", self.block_source(no))
                    }
                ),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn source(&self) -> String {
        self.functions
            .iter()
            .map(|f| {
                format!(
                    "fn {}({}) -> {} {{ {} }}",
                    f.name,
                    f.params
                        .iter()
                        .map(|(name, ty)| format!("{name}: {ty}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    f.ty,
                    self.block_source(&f.body)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
fn bool_expr(value: bool) -> Expr {
    Expr::Literal(Value::Bool(value))
}
fn name(value: &'static str) -> Expr {
    Expr::Name(value)
}
fn ret(value: Expr) -> Stmt {
    Stmt::Return(Some(value))
}
fn call(id: usize, args: Vec<Expr>) -> Expr {
    Expr::Call(id, args)
}
fn definition(
    name: &str,
    params: Vec<(&'static str, &'static str)>,
    ty: &'static str,
    body: Vec<Stmt>,
) -> Definition {
    Definition {
        name: name.into(),
        params,
        ty,
        body,
    }
}
fn compare(model: Model) -> Vec<String> {
    let source = model.source();
    let main = model.functions.len() - 1;
    let mut expected_trace = vec![];
    let expected = model.call(main, vec![], &mut expected_trace, 0);
    let (_, verified, entry) = compiled(&source);
    let mut actual_trace = vec![];
    let actual = execute(&verified, entry, &[], Limits::default(), &mut |event| {
        actual_trace.push(match event {
            Event::Enter(id) => format!("enter {}", model.functions[id.0].name),
            Event::Branch(id, value) => format!("branch {} {value}", model.functions[id.0].name),
            Event::Return(id, value) => format!("return {} {value}", model.functions[id.0].name),
        });
    })
    .unwrap();
    assert_eq!(actual.to_string(), expected.to_string(), "{source}");
    assert_eq!(actual_trace, expected_trace, "{source}");
    expected_trace
}
#[test]
fn generated_truth_tables_match_independent_values_and_call_branch_return_traces() {
    for mask in 0u8..16 {
        for a in [false, true] {
            for b in [false, true] {
                // Every two-input boolean truth table. Helper arguments contain
                // distinct calls; model traces expose reordering/duplication.
                let bit = |index: u32| bool_expr(mask & (1u8 << index) != 0u8);
                let branch_body = vec![Stmt::If(
                    name("a"),
                    vec![Stmt::If(name("b"), vec![ret(bit(3))], vec![ret(bit(2))])],
                    vec![Stmt::If(name("b"), vec![ret(bit(1))], vec![ret(bit(0))])],
                )];
                let model = Model {
                    functions: vec![
                        definition("left", vec![("x", "bool")], "bool", vec![ret(name("x"))]),
                        definition(
                            "right",
                            vec![("x", "bool")],
                            "bool",
                            vec![ret(Expr::Group(Box::new(name("x"))))],
                        ),
                        definition(
                            "table",
                            vec![("a", "bool"), ("b", "bool")],
                            "bool",
                            branch_body,
                        ),
                        definition(
                            "main",
                            vec![],
                            "bool",
                            vec![
                                Stmt::Let("saved", mask % 2 == 0, bool_expr(a)),
                                ret(call(
                                    2,
                                    vec![call(0, vec![name("saved")]), call(1, vec![bool_expr(b)])],
                                )),
                            ],
                        ),
                    ],
                };
                let mut expected_trace = vec![];
                assert_eq!(
                    model.call(3, vec![], &mut expected_trace, 0),
                    Value::Bool(mask & (1 << (usize::from(a) * 2 + usize::from(b))) != 0)
                );
                compare(model);
            }
        }
    }
}
#[test]
fn generated_mixed_return_scope_group_unit_and_discard_paths_match_oracle() {
    for early in [false, true] {
        for nested in [false, true] {
            let model = Model {
                functions: vec![
                    definition(
                        "unit",
                        vec![("value", "()")],
                        "()",
                        vec![
                            Stmt::Let("copy", false, Expr::Group(Box::new(name("value")))),
                            ret(name("copy")),
                        ],
                    ),
                    definition(
                        "id",
                        vec![("value", "bool")],
                        "bool",
                        vec![ret(name("value"))],
                    ),
                    definition(
                        "mixed",
                        vec![("first", "bool"), ("second", "bool")],
                        "bool",
                        vec![
                            Stmt::If(
                                call(1, vec![name("first")]),
                                vec![ret(bool_expr(false))],
                                vec![],
                            ),
                            Stmt::If(
                                name("second"),
                                vec![
                                    Stmt::Let("scoped", true, bool_expr(true)),
                                    Stmt::Discard(call(1, vec![name("scoped")])),
                                ],
                                vec![],
                            ),
                            Stmt::Let("scoped", false, name("second")),
                            ret(name("scoped")),
                        ],
                    ),
                    definition(
                        "main",
                        vec![],
                        "()",
                        vec![
                            Stmt::Discard(call(2, vec![bool_expr(early), bool_expr(nested)])),
                            Stmt::Discard(call(0, vec![Expr::Literal(Value::Unit)])),
                            Stmt::Return(None),
                        ],
                    ),
                ],
            };
            let trace = compare(model);
            assert!(trace.iter().any(|line| line == "return unit ()"));
        }
    }
}
