
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::{CStr, CString};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    line: usize,
    col: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Identifier(String),
    String(String),
    Number(f64),
    Fn,
    Async,
    Await,
    Let,
    Const,
    If,
    Else,
    While,
    Return,
    True,
    False,
    Print,
    Use,
    And,
    Or,
    Null,
    Eof,
}

#[derive(Clone, Debug)]
enum Literal {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Clone, Debug)]
enum Expr {
    Literal(Literal),
    Variable(String),
    Assign(String, Box<Expr>),
    AssignIndex(Box<Expr>, Box<Expr>, Box<Expr>),
    Unary(TokenKind, Box<Expr>),
    Binary(Box<Expr>, TokenKind, Box<Expr>),
    Logical(Box<Expr>, TokenKind, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Grouping(Box<Expr>),
    Await(Box<Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Let(String, Expr),
    Const(String, Expr),
    Print(Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Return(Option<Expr>),
    Use(String),
}

#[derive(Clone, Debug)]
struct Program {
    stmts: Vec<Stmt>,
    has_executable_top_level: bool,
}

#[derive(Clone)]
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Function(Rc<FunctionValue>),
    Task(Rc<TaskValue>),
    NativeFunction(NativeFunction),
}

type NativeFunction = fn(Vec<Value>) -> Result<Value, RuntimeError>;

#[derive(Clone)]
struct FunctionValue {
    name: String,
    params: Vec<String>,
    body: Vec<Stmt>,
    closure: EnvRef,
    is_async: bool,
}

#[derive(Clone)]
struct TaskValue {
    function: Rc<FunctionValue>,
    args: Vec<Value>,
}

type EnvRef = Rc<RefCell<Environment>>;

#[derive(Clone)]
struct Environment {
    values: HashMap<String, Value>,
    enclosing: Option<EnvRef>,
}

#[derive(Debug)]
enum RuntimeError {
    Message(String),
    Return(Value),
}

struct Interpreter {
    globals: EnvRef,
    loaded_modules: HashSet<PathBuf>,
    consts: HashSet<String>,
}

impl Interpreter {
    fn new() -> Self {
        let globals = Rc::new(RefCell::new(Environment::new(None)));
        {
            let mut g = globals.borrow_mut();
            g.define("clock".to_string(), Value::NativeFunction(native_clock));
            g.define("len".to_string(), Value::NativeFunction(native_len));
            g.define("push".to_string(), Value::NativeFunction(native_push));
            g.define("pop".to_string(), Value::NativeFunction(native_pop));
            g.define("range".to_string(), Value::NativeFunction(native_range));
            g.define("str".to_string(), Value::NativeFunction(native_str));
            g.define("c_len".to_string(), Value::NativeFunction(native_c_len));
            g.define("c_hash".to_string(), Value::NativeFunction(native_c_hash));
            g.define("cpp_len".to_string(), Value::NativeFunction(native_cpp_len));
            g.define("cpp_hash".to_string(), Value::NativeFunction(native_cpp_hash));
            g.define("assert".to_string(), Value::NativeFunction(native_assert));
            g.define("type_of".to_string(), Value::NativeFunction(native_type_of));
            g.define("sleep".to_string(), Value::NativeFunction(native_sleep));
        }
        Self { globals, loaded_modules: HashSet::new(), consts: HashSet::new() }
    }

    fn execute_program(&mut self, program: &Program, base_dir: &Path) -> Result<(), String> {
        let env = self.globals.clone();
        for stmt in &program.stmts {
            self.execute_stmt(stmt, env.clone(), base_dir)?;
        }

        if self.has_function("main") {
            let result = self.call_named_function("main", Vec::new())?;
            let result = match result {
                Value::Task(task) => self.execute_task(task.clone())?,
                other => other,
            };
            if !matches!(result, Value::Null) {
                println!("{}", result);
            }
        }

        Ok(())
    }

    fn has_function(&self, name: &str) -> bool {
        match self.globals.borrow().values.get(name) {
            Some(Value::Function(_)) => true,
            Some(Value::NativeFunction(_)) => true,
            _ => false,
        }
    }

    fn call_named_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let value = self.get_var(name).map_err(|e| e.to_string())?;
        self.call_value(value, args)
    }

    fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, String> {
        match callee {
            Value::Function(func) => {
                if func.is_async {
                    Ok(Value::Task(Rc::new(TaskValue { function: func.clone(), args })))
                } else {
                    self.invoke_function(func, args)
                }
            }
            Value::Task(task) => self.execute_task(task.clone()),
            Value::NativeFunction(f) => f(args).map_err(|e| match e {
                RuntimeError::Message(msg) => msg,
                RuntimeError::Return(_) => "Internal error: a native function triggered `return`.".to_string(),
            }),
            _ => Err("Value is not callable.".to_string()),
        }
    }

    fn invoke_function(&mut self, func: Rc<FunctionValue>, args: Vec<Value>) -> Result<Value, String> {
        if args.len() != func.params.len() {
            return Err(format!(
                "Function `{}` expected {} arguments but received {}.",
                func.name,
                func.params.len(),
                args.len()
            ));
        }
        let env = Rc::new(RefCell::new(Environment::new(Some(func.closure.clone()))));
        {
            let mut e = env.borrow_mut();
            for (name, value) in func.params.iter().cloned().zip(args.into_iter()) {
                e.define(name, value);
            }
        }
        match self.execute_block(&func.body, env, Path::new(".")) {
            Ok(()) => Ok(Value::Null),
            Err(RuntimeError::Return(v)) => Ok(v),
            Err(RuntimeError::Message(msg)) => Err(msg),
        }
    }

    fn execute_task(&mut self, task: Rc<TaskValue>) -> Result<Value, String> {
        self.invoke_function(task.function.clone(), task.args.clone())
    }

    fn execute_stmt(&mut self, stmt: &Stmt, env: EnvRef, base_dir: &Path) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Let(name, expr) => {
                let value = self.evaluate(expr, env.clone(), base_dir).map_err(RuntimeError::Message)?;
                env.borrow_mut().define(name.clone(), value);
                Ok(())
            }
            Stmt::Const(name, expr) => {
                let value = self.evaluate(expr, env.clone(), base_dir).map_err(RuntimeError::Message)?;
                env.borrow_mut().define(name.clone(), value);
                self.consts.insert(name.clone());
                Ok(())
            }
            Stmt::Print(expr) => {
                let value = self.evaluate(expr, env, base_dir).map_err(RuntimeError::Message)?;
                println!("{}", value);
                Ok(())
            }
            Stmt::Expr(expr) => {
                let _ = self.evaluate(expr, env, base_dir).map_err(RuntimeError::Message)?;
                Ok(())
            }
            Stmt::Block(stmts) => {
                self.execute_block(stmts, Rc::new(RefCell::new(Environment::new(Some(env)))), base_dir)
            }
            Stmt::If { cond, then_branch, else_branch } => {
                let cond_value = self.evaluate(cond, env.clone(), base_dir).map_err(RuntimeError::Message)?;
                if self.is_truthy(&cond_value) {
                    self.execute_stmt(then_branch, env, base_dir)
                } else if let Some(other) = else_branch {
                    self.execute_stmt(other, env, base_dir)
                } else {
                    Ok(())
                }
            }
            Stmt::While { cond, body } => {
                while self.is_truthy(&self.evaluate(cond, env.clone(), base_dir).map_err(RuntimeError::Message)?) {
                    self.execute_stmt(body, env.clone(), base_dir)?;
                }
                Ok(())
            }
            Stmt::Function { name, params, body, is_async } => {
                let func = FunctionValue {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                    is_async: *is_async,
                };
                env.borrow_mut().define(name.clone(), Value::Function(Rc::new(func)));
                Ok(())
            }
            Stmt::Return(expr) => {
                let value = if let Some(expr) = expr {
                    self.evaluate(expr, env, base_dir).map_err(RuntimeError::Message)?
                } else {
                    Value::Null
                };
                Err(RuntimeError::Return(value))
            }
            Stmt::Use(path) => {
                self.execute_module(path, base_dir)?;
                Ok(())
            }
        }
    }

    fn execute_block(&mut self, stmts: &[Stmt], env: EnvRef, base_dir: &Path) -> Result<(), RuntimeError> {
        for stmt in stmts {
            match self.execute_stmt(stmt, env.clone(), base_dir) {
                Ok(()) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    fn execute_module(&mut self, path_text: &str, base_dir: &Path) -> Result<(), RuntimeError> {
        let path = resolve_path(base_dir, path_text);
        let canonical = fs::canonicalize(&path).map_err(|e| RuntimeError::Message(format!("Cannot open module: {} ({})", path.display(), e)))?;
        if self.loaded_modules.contains(&canonical) {
            return Ok(())
        }
        self.loaded_modules.insert(canonical.clone());

        let source = fs::read_to_string(&canonical)
            .map_err(|e| RuntimeError::Message(format!("Cannot read module: {} ({})", canonical.display(), e)))?;
        let source = preprocess_source(&source).map_err(RuntimeError::Message)?;
        let mut parser = Parser::new(&source);
        let program = parser.parse_program().map_err(RuntimeError::Message)?;
        let parent = canonical.parent().unwrap_or(base_dir);
        for stmt in &program.stmts {
            self.execute_stmt(stmt, self.globals.clone(), parent)?;
        }
        Ok(())
    }

    fn evaluate(&mut self, expr: &Expr, env: EnvRef, base_dir: &Path) -> Result<Value, String> {
        match expr {
            Expr::Literal(Literal::Number(n)) => Ok(Value::Number(*n)),
            Expr::Literal(Literal::String(s)) => Ok(Value::String(s.clone())),
            Expr::Literal(Literal::Bool(b)) => Ok(Value::Bool(*b)),
            Expr::Literal(Literal::Null) => Ok(Value::Null),
            Expr::Variable(name) => self.get_var_scoped(name, env).map_err(|e| e.to_string()),
            Expr::Assign(name, value_expr) => {
                let value = self.evaluate(value_expr, env.clone(), base_dir)?;
                self.assign_var(name, value.clone(), env)?;
                Ok(value)
            }
            Expr::AssignIndex(target, index, value_expr) => {
                let target_value = self.evaluate(target, env.clone(), base_dir)?;
                let index_value = self.evaluate(index, env.clone(), base_dir)?;
                let value = self.evaluate(value_expr, env, base_dir)?;
                self.assign_index(target_value, index_value, value.clone())?;
                Ok(value)
            }
            Expr::Grouping(inner) => self.evaluate(inner, env, base_dir),
            Expr::Await(inner) => {
                let value = self.evaluate(inner, env, base_dir)?;
                match value {
                    Value::Task(task) => self.execute_task(task),
                    other => Ok(other),
                }
            }
            Expr::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.evaluate(item, env.clone(), base_dir)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::Index(target, index) => {
                let target_value = self.evaluate(target, env.clone(), base_dir)?;
                let index_value = self.evaluate(index, env, base_dir)?;
                self.index_value(target_value, index_value)
            }
            Expr::Unary(op, right) => {
                let right = self.evaluate(right, env, base_dir)?;
                match op {
                    TokenKind::Minus => match right {
                        Value::Number(n) => Ok(Value::Number(-n)),
                        _ => Err("Unary `-` can only be used with numbers.".to_string()),
                    },
                    TokenKind::Bang => Ok(Value::Bool(!self.is_truthy(&right))),
                    _ => Err("Unknown unary operator.".to_string()),
                }
            }
            Expr::Binary(left, op, right) => {
                let left = self.evaluate(left, env.clone(), base_dir)?;
                let right = self.evaluate(right, env, base_dir)?;
                self.eval_binary(left, op, right)
            }
            Expr::Logical(left, op, right) => {
                let left = self.evaluate(left, env.clone(), base_dir)?;
                match op {
                    TokenKind::Or => {
                        if self.is_truthy(&left) {
                            Ok(left)
                        } else {
                            self.evaluate(right, env, base_dir)
                        }
                    }
                    TokenKind::And => {
                        if !self.is_truthy(&left) {
                            Ok(left)
                        } else {
                            self.evaluate(right, env, base_dir)
                        }
                    }
                    _ => Err("Unknown logical operator.".to_string()),
                }
            }
            Expr::Call(callee, args) => {
                let callee_value = self.evaluate(callee, env.clone(), base_dir)?;
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.evaluate(arg, env.clone(), base_dir)?);
                }
                self.call_value(callee_value, values)
            }
        }
    }

    fn eval_binary(&self, left: Value, op: &TokenKind, right: Value) -> Result<Value, String> {
        match op {
            TokenKind::Plus => match (left, right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                (Value::String(a), b) => Ok(Value::String(a + &b.to_string())),
                (a, Value::String(b)) => Ok(Value::String(a.to_string() + &b)),
                _ => Err("`+` can be used with numbers or strings.".to_string()),
            },
            TokenKind::Minus => arithmetic(left, right, |a, b| a - b),
            TokenKind::Star => arithmetic(left, right, |a, b| a * b),
            TokenKind::Slash => arithmetic(left, right, |a, b| a / b),
            TokenKind::Greater => compare(left, right, |a, b| a > b),
            TokenKind::GreaterEqual => compare(left, right, |a, b| a >= b),
            TokenKind::Less => compare(left, right, |a, b| a < b),
            TokenKind::LessEqual => compare(left, right, |a, b| a <= b),
            TokenKind::EqualEqual => Ok(Value::Bool(values_equal(&left, &right))),
            TokenKind::BangEqual => Ok(Value::Bool(!values_equal(&left, &right))),
            _ => Err("Unknown binary operator.".to_string()),
        }
    }

    fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => !arr.borrow().is_empty(),
            Value::Function(_) | Value::Task(_) | Value::NativeFunction(_) => true,
        }
    }

    fn get_var(&self, name: &str) -> Result<Value, String> {
        self.globals
            .borrow()
            .values
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Undefined identifier: {}", name))
    }

    fn get_var_scoped(&self, name: &str, env: EnvRef) -> Result<Value, String> {
        Environment::get_scoped(&env, name).ok_or_else(|| format!("Undefined identifier: {}", name))
    }

    fn assign_var(&self, name: &str, value: Value, env: EnvRef) -> Result<(), String> {
        if self.consts.contains(name) {
            return Err(format!("Cannot reassign constant `{}`.", name));
        }
        Environment::assign_scoped(&env, name, value)
            .ok_or_else(|| format!("Assignment target not found: {}", name))
    }

    fn index_value(&self, target: Value, index: Value) -> Result<Value, String> {
        let idx = as_index(&index)?;
        match target {
            Value::Array(items) => items
                .borrow()
                .get(idx)
                .cloned()
                .ok_or_else(|| format!("Array index {} is out of bounds.", idx)),
            Value::String(text) => text
                .chars()
                .nth(idx)
                .map(|c| Value::String(c.to_string()))
                .ok_or_else(|| format!("String index {} is out of bounds.", idx)),
            _ => Err("Index access can only be used on arrays or strings.".to_string()),
        }
    }

    fn assign_index(&self, target: Value, index: Value, value: Value) -> Result<(), String> {
        let idx = as_index(&index)?;
        match target {
            Value::Array(items) => {
                let mut items = items.borrow_mut();
                if idx >= items.len() {
                    return Err(format!("Array index {} is out of bounds.", idx));
                }
                items[idx] = value;
                Ok(())
            }
            _ => Err("Indexed assignment can only be used on arrays.".to_string()),
        }
    }
}

impl Environment {
    fn new(enclosing: Option<EnvRef>) -> Self {
        Self { values: HashMap::new(), enclosing }
    }

    fn define(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    fn get_scoped(env: &EnvRef, name: &str) -> Option<Value> {
        if let Some(value) = env.borrow().values.get(name) {
            return Some(value.clone());
        }
        let parent = env.borrow().enclosing.clone();
        parent.and_then(|p| Environment::get_scoped(&p, name))
    }

    fn assign_scoped(env: &EnvRef, name: &str, value: Value) -> Option<()> {
        if env.borrow().values.contains_key(name) {
            env.borrow_mut().values.insert(name.to_string(), value);
            return Some(());
        }
        let parent = env.borrow().enclosing.clone();
        if let Some(parent) = parent {
            return Environment::assign_scoped(&parent, name, value);
        }
        None
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{:.0}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::String(s) => write!(f, "{}", s),
            Value::Array(items) => {
                let items = items.borrow();
                let rendered = items.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "[{}]", rendered)
            }
            Value::Function(func) => write!(f, "<fn {}>", func.name),
            Value::Task(task) => write!(f, "<task {}>", task.function.name),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}

fn arithmetic(left: Value, right: Value, op: fn(f64, f64) -> f64) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(op(a, b))),
        _ => Err("Arithmetic operations require numbers.".to_string()),
    }
}

fn compare(left: Value, right: Value, op: fn(f64, f64) -> bool) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err("Comparison operations require numbers.".to_string()),
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        _ => false,
    }
}

#[derive(Clone, Debug)]
struct MacroDef {
    params: Vec<String>,
    body: String,
}

fn preprocess_source(source: &str) -> Result<String, String> {
    let mut macros: HashMap<String, MacroDef> = HashMap::new();
    let mut body_lines = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.starts_with("macro ") {
            let def = parse_macro_def(line)?;
            macros.insert(def.0, def.1);
        } else {
            body_lines.push(raw_line.to_string());
        }
    }

    let mut out = body_lines.join("\n");
    for _ in 0..16 {
        let (next, changed) = expand_macro_pass(&out, &macros)?;
        out = next;
        if !changed {
            break;
        }
    }
    Ok(out)
}

fn parse_macro_def(line: &str) -> Result<(String, MacroDef), String> {
    // Syntax: macro name(a, b) => body;
    let rest = line.strip_prefix("macro ").ok_or_else(|| "Invalid macro definition syntax.".to_string())?;
    let (head, body) = rest.split_once("=>").ok_or_else(|| "A macro definition requires `=>`.".to_string())?;
    let head = head.trim();
    let body = body.trim().trim_end_matches(';').trim();
    let open = head.find('(').ok_or_else(|| "A macro definition requires `(`.".to_string())?;
    let close = head.rfind(')').ok_or_else(|| "A macro definition requires `)`.".to_string())?;
    if close <= open {
        return Err("Invalid macro parameter list.".to_string());
    }
    let name = head[..open].trim();
    if name.is_empty() {
        return Err("Macro name is empty.".to_string());
    }
    let params = head[open + 1..close]
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    Ok((name.to_string(), MacroDef { params, body: body.to_string() }))
}

fn expand_macro_pass(source: &str, macros: &HashMap<String, MacroDef>) -> Result<(String, bool), String> {
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut changed = false;

    while i < chars.len() {
        let c = chars[i];
        if is_alpha(c) {
            let start = i;
            i += 1;
            while i < chars.len() && is_alpha_numeric(chars[i]) {
                i += 1;
            }
            let ident = chars[start..i].iter().collect::<String>();
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if let Some(def) = macros.get(&ident) {
                if j < chars.len() && chars[j] == '(' {
                    let (args, end) = parse_macro_args(&chars, j)?;
                    out.push_str(&expand_macro(def, &args)?);
                    i = end;
                    changed = true;
                    continue;
                }
            }
            out.push_str(&ident);
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok((out, changed))
}

fn parse_macro_args(chars: &[char], open_idx: usize) -> Result<(Vec<String>, usize), String> {
    let mut depth = 0usize;
    let mut i = open_idx;
    let mut current = String::new();
    let mut args = Vec::new();
    i += 1; // skip '('
    depth += 1;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '(' => { depth += 1; current.push(c); }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current.trim().to_string());
                    return Ok((args, i + 1));
                }
                current.push(c);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
        i += 1;
    }
    Err("Macro call is missing a closing parenthesis.".to_string())
}

fn expand_macro(def: &MacroDef, args: &[String]) -> Result<String, String> {
    if args.len() != def.params.len() {
        return Err(format!("Macro argument count mismatch: expected {}, got {}", def.params.len(), args.len()));
    }
    let mut body = def.body.clone();
    for (param, arg) in def.params.iter().zip(args.iter()) {
        body = body.replace(param, arg);
    }
    Ok(body)
}


struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self { tokens: Lexer::new(source).lex(), current: 0 }
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut stmts = Vec::new();
        let mut has_executable_top_level = false;
        while !self.is_at_end() {
            let stmt = self.declaration()?;
            if matches!(stmt, Stmt::Print(_) | Stmt::Expr(_) | Stmt::If { .. } | Stmt::While { .. } | Stmt::Return(_) | Stmt::Block(_)) {
                has_executable_top_level = true;
            }
            stmts.push(stmt);
        }
        Ok(Program { stmts, has_executable_top_level })
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        let is_async = self.match_simple(&[TokenKind::Async]);
        if self.match_simple(&[TokenKind::Fn]) {
            return self.function_decl(is_async);
        }
        if self.match_simple(&[TokenKind::Const]) {
            return self.const_decl();
        }
        if self.match_simple(&[TokenKind::Let]) {
            return self.let_decl();
        }
        if self.match_simple(&[TokenKind::Use]) {
            return self.use_decl();
        }
        if is_async {
            return Err("`async` must be used with a function definition.".to_string());
        }
        self.statement()
    }

    fn function_decl(&mut self, is_async: bool) -> Result<Stmt, String> {
        let name = self.consume_identifier("Function name is required.")?;
        self.consume_simple(TokenKind::LeftParen, "A function definition requires `(`.")?;
        let mut params = Vec::new();
        if !self.check_simple(&TokenKind::RightParen) {
            loop {
                params.push(self.consume_identifier("Parameter name is required.")?);
                if !self.match_simple(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume_simple(TokenKind::RightParen, "A function definition requires `)`.")?;
        self.consume_simple(TokenKind::LeftBrace, "A function body requires `{`.")?;
        let body = self.block_stmts()?;
        Ok(Stmt::Function { name, params, body, is_async })
    }

    fn const_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("Constant name is required.")?;
        self.consume_simple(TokenKind::Equal, "A constant declaration requires `=`.")?;
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "A constant declaration must end with `;`.")?;
        Ok(Stmt::Const(name, expr))
    }

    fn let_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("Variable name is required.")?;
        self.consume_simple(TokenKind::Equal, "Assignment requires `=`.")?;
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "A declaration must end with `;`.")?;
        Ok(Stmt::Let(name, expr))
    }

    fn use_decl(&mut self) -> Result<Stmt, String> {
        let path = match self.advance().kind.clone() {
            TokenKind::String(s) => s,
            other => return Err(format!("`use` requires a string path. Found token: {:?}", other)),
        };
        self.consume_simple(TokenKind::Semicolon, "`use` must end with `;`.")?;
        Ok(Stmt::Use(path))
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.match_simple(&[TokenKind::Print]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`print` must end with `;`.")?;
            return Ok(Stmt::Print(expr));
        }
        if self.match_simple(&[TokenKind::Return]) {
            if self.check_simple(&TokenKind::Semicolon) {
                self.advance();
                return Ok(Stmt::Return(None));
            }
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`return` must end with `;`.")?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.match_simple(&[TokenKind::If]) {
            return self.if_stmt();
        }
        if self.match_simple(&[TokenKind::While]) {
            return self.while_stmt();
        }
        if self.match_simple(&[TokenKind::LeftBrace]) {
            let body = self.block_stmts()?;
            return Ok(Stmt::Block(body));
        }
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "An expression statement must end with `;`.")?;
        Ok(Stmt::Expr(expr))
    }

    fn if_stmt(&mut self) -> Result<Stmt, String> {
        self.consume_simple(TokenKind::LeftParen, "An `if` condition requires `(`.")?;
        let cond = self.expression()?;
        self.consume_simple(TokenKind::RightParen, "An `if` condition requires `)`.")?;
        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_simple(&[TokenKind::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If { cond, then_branch, else_branch })
    }

    fn while_stmt(&mut self) -> Result<Stmt, String> {
        self.consume_simple(TokenKind::LeftParen, "A `while` condition requires `(`.")?;
        let cond = self.expression()?;
        self.consume_simple(TokenKind::RightParen, "A `while` condition requires `)`.")?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::While { cond, body })
    }

    fn block_stmts(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.check_simple(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.declaration()?);
        }
        self.consume_simple(TokenKind::RightBrace, "A block must end with `}`.")?;
        Ok(stmts)
    }

    fn expression(&mut self) -> Result<Expr, String> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, String> {
        let expr = self.or()?;
        if self.match_simple(&[TokenKind::Equal]) {
            let value = self.assignment()?;
            return match expr {
                Expr::Variable(name) => Ok(Expr::Assign(name, Box::new(value))),
                Expr::Index(target, index) => Ok(Expr::AssignIndex(target, index, Box::new(value))),
                _ => Err("An assignment target must be an identifier or array index.".to_string()),
            };
        }
        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut expr = self.and()?;
        while self.match_simple(&[TokenKind::Or]) {
            let op = TokenKind::Or;
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;
        while self.match_simple(&[TokenKind::And]) {
            let op = TokenKind::And;
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;
        while self.match_simple(&[TokenKind::BangEqual, TokenKind::EqualEqual]) {
            let op = self.previous().kind.clone();
            let right = self.comparison()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;
        while self.match_simple(&[
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
        ]) {
            let op = self.previous().kind.clone();
            let right = self.term()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        while self.match_simple(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let right = self.factor()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;
        while self.match_simple(&[TokenKind::Star, TokenKind::Slash]) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::Await]) {
            let right = self.unary()?;
            return Ok(Expr::Await(Box::new(right)));
        }
        if self.match_simple(&[TokenKind::Bang, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            return Ok(Expr::Unary(op, Box::new(right)));
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        loop {
            if self.match_simple(&[TokenKind::LeftParen]) {
                let mut args = Vec::new();
                if !self.check_simple(&TokenKind::RightParen) {
                    loop {
                        args.push(self.expression()?);
                        if !self.match_simple(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume_simple(TokenKind::RightParen, "A call expression requires `)`.")?;
                expr = Expr::Call(Box::new(expr), args);
                continue;
            }
            if self.match_simple(&[TokenKind::LeftBracket]) {
                let index = self.expression()?;
                self.consume_simple(TokenKind::RightBracket, "An index expression requires `]`.")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::False]) {
            return Ok(Expr::Literal(Literal::Bool(false)));
        }
        if self.match_simple(&[TokenKind::True]) {
            return Ok(Expr::Literal(Literal::Bool(true)));
        }
        if self.match_simple(&[TokenKind::Null]) {
            return Ok(Expr::Literal(Literal::Null));
        }
        if self.check_number() {
            if let TokenKind::Number(n) = self.advance().kind.clone() {
                return Ok(Expr::Literal(Literal::Number(n)));
            }
        }
        if self.check_string() {
            if let TokenKind::String(s) = self.advance().kind.clone() {
                return Ok(Expr::Literal(Literal::String(s)));
            }
        }
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() {
                return Ok(Expr::Variable(name));
            }
        }
        if self.match_simple(&[TokenKind::LeftBracket]) {
            let mut items = Vec::new();
            if !self.check_simple(&TokenKind::RightBracket) {
                loop {
                    items.push(self.expression()?);
                    if !self.match_simple(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            self.consume_simple(TokenKind::RightBracket, "An array literal must end with `]`.")?;
            return Ok(Expr::Array(items));
        }
        if self.match_simple(&[TokenKind::LeftParen]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::RightParen, "Grouping requires `)`.")?;
            return Ok(Expr::Grouping(Box::new(expr)));
        }
        if self.match_simple(&[TokenKind::Eof]) {
            return Err("Unexpected EOF.".to_string());
        }
        Err(format!("Token cannot be parsed as an expression: {:?}", self.peek().kind))
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() {
                return Ok(name);
            }
        }
        Err(self.error_here(message))
    }

    fn consume_simple(&mut self, kind: TokenKind, message: &str) -> Result<(), String> {
        if self.check_simple(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn error_here(&self, message: &str) -> String {
        let t = self.peek();
        format!("{} (line {}, col {})", message, t.line, t.col)
    }

    fn match_simple(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check_simple(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check_simple(&self, kind: &TokenKind) -> bool {
        use TokenKind::*;
        match (kind, &self.peek().kind) {
            (LeftParen, LeftParen)
            | (RightParen, RightParen)
            | (LeftBrace, LeftBrace)
            | (RightBrace, RightBrace)
            | (Comma, Comma)
            | (Dot, Dot)
            | (Minus, Minus)
            | (Plus, Plus)
            | (Semicolon, Semicolon)
            | (Slash, Slash)
            | (Star, Star)
            | (LeftBracket, LeftBracket)
            | (RightBracket, RightBracket)
            | (Bang, Bang)
            | (BangEqual, BangEqual)
            | (Equal, Equal)
            | (EqualEqual, EqualEqual)
            | (Greater, Greater)
            | (GreaterEqual, GreaterEqual)
            | (Less, Less)
            | (LessEqual, LessEqual)
            | (Fn, Fn)
            | (Async, Async)
            | (Await, Await)
            | (Let, Let)
            | (Const, Const)
            | (If, If)
            | (Else, Else)
            | (While, While)
            | (Return, Return)
            | (True, True)
            | (False, False)
            | (Print, Print)
            | (Use, Use)
            | (And, And)
            | (Or, Or)
            | (Null, Null)
            | (Eof, Eof) => true,
            _ => false,
        }
    }

    fn check_identifier(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Identifier(_))
    }

    fn check_number(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Number(_))
    }

    fn check_string(&self) -> bool {
        matches!(self.peek().kind, TokenKind::String(_))
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            start: 0,
            current: 0,
            line: 1,
            col: 1,
        }
    }

    fn lex(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while !self.is_at_end() {
            self.start = self.current;
            if let Some(token) = self.scan_token() {
                tokens.push(token);
            }
        }
        tokens.push(Token { kind: TokenKind::Eof, line: self.line, col: self.col });
        tokens
    }

    fn scan_token(&mut self) -> Option<Token> {
        let c = self.advance();
        let line = self.line;
        let col = self.col.saturating_sub(1);
        use TokenKind::*;
        let kind = match c {
            '(' => LeftParen,
            ')' => RightParen,
            '{' => LeftBrace,
            '}' => RightBrace,
            '[' => LeftBracket,
            ']' => RightBracket,
            ',' => Comma,
            '.' => Dot,
            '-' => Minus,
            '+' => Plus,
            ';' => Semicolon,
            '*' => Star,
            '!' => {
                if self.match_char('=') { BangEqual } else { Bang }
            }
            '=' => {
                if self.match_char('=') { EqualEqual } else { Equal }
            }
            '<' => {
                if self.match_char('=') { LessEqual } else { Less }
            }
            '>' => {
                if self.match_char('=') { GreaterEqual } else { Greater }
            }
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                    return None;
                } else if self.match_char('*') {
                    self.block_comment();
                    return None;
                } else {
                    Slash
                }
            }
            ' ' | '\r' | '\t' => return None,
            '\n' => {
                self.line += 1;
                self.col = 1;
                return None;
            }
            '"' => {
                return Some(self.string_token(line, col));
            }
            c if c.is_ascii_digit() => {
                return Some(self.number_token(c, line, col));
            }
            c if is_alpha(c) => {
                return Some(self.identifier_token(c, line, col));
            }
            _ => {
                panic!("Unknown character found: '{}' (line {}, col {})", c, line, col);
            }
        };
        Some(Token { kind, line, col })
    }

    fn block_comment(&mut self) {
        while !self.is_at_end() {
            if self.peek() == '*' && self.peek_next() == '/' {
                self.advance();
                self.advance();
                return;
            }
            if self.peek() == '\n' {
                self.line += 1;
                self.col = 1;
            }
            self.advance();
        }
    }

    fn string_token(&mut self, line: usize, col: usize) -> Token {
        let mut value = String::new();
        while !self.is_at_end() && self.peek() != '"' {
            let c = self.advance();
            if c == '\\' && !self.is_at_end() {
                let escaped = self.advance();
                let translated = match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                };
                value.push(translated);
            } else {
                if c == '\n' {
                    self.line += 1;
                    self.col = 1;
                }
                value.push(c);
            }
        }
        if self.is_at_end() {
            return Token { kind: TokenKind::String(value), line, col };
        }
        self.advance();
        Token { kind: TokenKind::String(value), line, col }
    }

    fn number_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while self.peek().is_ascii_digit() {
            text.push(self.advance());
        }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            text.push(self.advance());
            while self.peek().is_ascii_digit() {
                text.push(self.advance());
            }
        }
        let value = text.parse::<f64>().unwrap_or(0.0);
        Token { kind: TokenKind::Number(value), line, col }
    }

    fn identifier_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while is_alpha_numeric(self.peek()) {
            text.push(self.advance());
        }
        let kind = match text.as_str() {
            "fn" => TokenKind::Fn,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "print" => TokenKind::Print,
            "use" => TokenKind::Use,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(text),
        };
        Token { kind, line, col }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.chars.len()
    }

    fn advance(&mut self) -> char {
        let ch = self.chars[self.current];
        self.current += 1;
        self.col += 1;
        ch
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.chars[self.current] != expected {
            return false;
        }
        self.current += 1;
        self.col += 1;
        true
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.current + 1]
        }
    }
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_alpha_numeric(c: char) -> bool {
    is_alpha(c) || c.is_ascii_digit()
}

fn resolve_path(base_dir: &Path, text: &str) -> PathBuf {
    let candidate = Path::new(text);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    }
}

fn native_clock(_: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RuntimeError::Message(format!("Failed to get clock: {}", e)))?;
    Ok(Value::Number(now.as_secs_f64()))
}

fn native_assert(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(RuntimeError::Message("assert requires at least 1 argument.".to_string()));
    }
    if args.len() == 1 {
        if truthy(&args[0]) {
            return Ok(Value::Null);
        }
        return Err(RuntimeError::Message("assert failed".to_string()));
    }
    if truthy(&args[0]) {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::Message(args[1].to_string()))
    }
}

fn native_type_of(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("type_of requires 1 argument.".to_string()));
    }
    let t = match &args[0] {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Function(_) => "function",
        Value::Task(_) => "task",
        Value::NativeFunction(_) => "native_function",
    };
    Ok(Value::String(t.to_string()))
}

fn native_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("len requires 1 argument.".to_string()));
    }
    match &args[0] {
        Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
        Value::Array(items) => Ok(Value::Number(items.borrow().len() as f64)),
        _ => Err(RuntimeError::Message("len can only be used with strings or arrays.".to_string())),
    }
}

fn native_push(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError::Message("push requires 2 arguments.".to_string()));
    }
    match &args[0] {
        Value::Array(items) => {
            items.borrow_mut().push(args[1].clone());
            Ok(Value::Null)
        }
        _ => Err(RuntimeError::Message("The first argument to push must be an array.".to_string())),
    }
}

fn native_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("pop requires 1 argument.".to_string()));
    }
    match &args[0] {
        Value::Array(items) => Ok(items.borrow_mut().pop().unwrap_or(Value::Null)),
        _ => Err(RuntimeError::Message("The argument to pop must be an array.".to_string())),
    }
}

fn native_range(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(RuntimeError::Message("range requires 2 arguments.".to_string()));
    }
    let start = match &args[0] {
        Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as usize,
        _ => return Err(RuntimeError::Message("range arguments must be non-negative integers.".to_string())),
    };
    let end = match &args[1] {
        Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as usize,
        _ => return Err(RuntimeError::Message("range arguments must be non-negative integers.".to_string())),
    };
    let values = (start..end).map(|n| Value::Number(n as f64)).collect::<Vec<_>>();
    Ok(Value::Array(Rc::new(RefCell::new(values))))
}

fn native_sleep(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("sleep requires 1 argument.".to_string()));
    }
    let ms = match &args[0] {
        Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 => *n as u64,
        _ => return Err(RuntimeError::Message("sleep requires a non-negative integer number of milliseconds.".to_string())),
    };
    thread::sleep(Duration::from_millis(ms));
    Ok(Value::Null)
}

fn native_str(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("str requires 1 argument.".to_string()));
    }
    Ok(Value::String(args[0].to_string()))
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.borrow().is_empty(),
        Value::Function(_) | Value::Task(_) | Value::NativeFunction(_) => true,
    }
}

fn as_index(value: &Value) -> Result<usize, String> {
    match value {
        Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
        _ => Err("Index must be a non-negative integer.".to_string()),
    }
}

fn native_cpp_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("cpp_len requires 1 string argument.".to_string()));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::Message("cpp_len requires a string argument.".to_string())),
    };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("String contains a NUL byte.".to_string()))?;
    let len = unsafe { oxid_cpp_len(cstr.as_ptr()) };
    Ok(Value::Number(len as f64))
}

fn native_cpp_hash(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("cpp_hash requires 1 string argument.".to_string()));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::Message("cpp_hash requires a string argument.".to_string())),
    };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("String contains a NUL byte.".to_string()))?;
    let hash = unsafe { oxid_cpp_hash(cstr.as_ptr()) };
    Ok(Value::String(format!("{:016x}", hash)))
}

fn native_c_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("c_len requires 1 string argument.".to_string()));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::Message("c_len requires a string argument.".to_string())),
    };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("String contains a NUL byte.".to_string()))?;
    let len = unsafe { oxid_c_strlen(cstr.as_ptr()) };
    Ok(Value::Number(len as f64))
}

fn native_c_hash(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::Message("c_hash requires 1 string argument.".to_string()));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::Message("c_hash requires a string argument.".to_string())),
    };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("String contains a NUL byte.".to_string()))?;
    let hash = unsafe { oxid_c_hash(cstr.as_ptr()) };
    Ok(Value::String(format!("{:016x}", hash)))
}

extern "C" {
    fn oxid_c_strlen(s: *const c_char) -> usize;
    fn oxid_c_hash(s: *const c_char) -> u64;
    fn oxid_cpp_len(s: *const c_char) -> usize;
    fn oxid_cpp_hash(s: *const c_char) -> u64;
}

fn run_source(source: &str, base_dir: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let source = preprocess_source(source)?;
    let mut parser = Parser::new(&source);
    let program = parser.parse_program()?;
    interp.execute_program(&program, base_dir)
}

fn run_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path)
        .map_err(|e| format!("Cannot open file: {} ({})", path.display(), e))?;
    let source = fs::read_to_string(&canonical)
        .map_err(|e| format!("Cannot read file: {} ({})", canonical.display(), e))?;
    let base_dir = canonical.parent().unwrap_or(Path::new("."));
    run_source(&source, base_dir, interp)
}

fn repl(interp: &mut Interpreter) -> Result<(), String> {
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("oxid> ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        line.clear();
        if stdin.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let src = line.trim();
        if src.is_empty() {
            continue;
        }
        if src == ":quit" || src == ":exit" {
            break;
        }
        match run_source(src, Path::new("."), interp) {
            Ok(()) => {}
            Err(err) => eprintln!("{}", err),
        }
    }
    Ok(())
}



#[derive(Clone, Debug, Default)]
struct ProjectManifest {
    name: Option<String>,
    version: Option<String>,
    entry: Option<String>,
}

fn parse_manifest_value(raw: &str) -> Option<String> {
    let value = raw.split_once('=').map(|(_, v)| v.trim()).unwrap_or(raw.trim());
    let trimmed = value.trim_end_matches(',').trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

fn load_manifest(path: &Path) -> Result<ProjectManifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read manifest: {} ({})", path.display(), e))?;
    let mut manifest = ProjectManifest::default();
    let mut section = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_start_matches('[').trim_end_matches(']').to_string();
            continue;
        }
        let Some((key, value_raw)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_manifest_value(value_raw).unwrap_or_else(|| value_raw.trim().trim_matches('"').to_string());

        match (section.as_str(), key) {
            ("project", "name") | ("", "name") => manifest.name = Some(value),
            ("project", "version") | ("", "version") => manifest.version = Some(value),
            ("project", "entry") | ("build", "entry") | ("", "entry") => manifest.entry = Some(value),
            _ => {}
        }
    }

    Ok(manifest)
}

fn collect_oxid_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(read_dir) = fs::read_dir(root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()) == Some("target") {
                continue;
            }
            if path.is_dir() {
                files.extend(collect_oxid_files(&path));
            } else if matches!(path.extension().and_then(|s| s.to_str()), Some("ox" | "toml" | "c" | "h" | "cpp" | "hpp")) {
                files.push(path);
            }
        }
    }
    files
}

fn latest_mtime(files: &[PathBuf]) -> SystemTime {
    let mut latest = SystemTime::UNIX_EPOCH;
    for file in files {
        if let Ok(meta) = fs::metadata(file) {
            if let Ok(modified) = meta.modified() {
                if modified > latest {
                    latest = modified;
                }
            }
        }
    }
    latest
}

fn watch_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("Cannot open file: {} ({})", path.display(), e))?;
    let root = canonical.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut files = collect_oxid_files(&root);
    if !files.contains(&canonical) {
        files.push(canonical.clone());
    }
    let mut last = latest_mtime(&files);
    println!("watching {} ... (Ctrl+C to stop)", canonical.display());
    loop {
        thread::sleep(Duration::from_millis(500));
        let current_files = collect_oxid_files(&root);
        let current_latest = latest_mtime(&current_files);
        if current_latest > last {
            last = current_latest;
            println!("reloading...");
            match run_file(&canonical, interp) {
                Ok(()) => println!("ok"),
                Err(err) => eprintln!("error: {}", err),
            }
        }
    }
}


fn build_project(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("oxid.toml");
    if !manifest_path.exists() {
        return Err(format!("Manifest not found: {}", manifest_path.display()));
    }
    let manifest = load_manifest(&manifest_path)?;
    let entry = manifest
        .entry
        .clone()
        .or_else(|| {
            let src_main = root.join("src/main.ox");
            if src_main.exists() { Some("src/main.ox".to_string()) } else { None }
        })
        .or_else(|| {
            let root_main = root.join("main.ox");
            if root_main.exists() { Some("main.ox".to_string()) } else { None }
        })
        .ok_or_else(|| "Entry file not found. Set `entry` in oxid.toml or create `src/main.ox`.".to_string())?;

    let entry_path = root.join(&entry);
    if !entry_path.exists() {
        return Err(format!("Entry file not found: {}", entry_path.display()));
    }

    let source = fs::read_to_string(&entry_path)
        .map_err(|e| format!("Cannot read file: {} ({})", entry_path.display(), e))?;
    let source = preprocess_source(&source)?;
    let mut parser = Parser::new(&source);
    parser.parse_program()?;

    let project_name = manifest.name.clone().unwrap_or_else(|| "unknown".to_string());
    let project_version = manifest.version.clone().unwrap_or_else(|| "unknown".to_string());
    println!("build ok: {} ({} {})", entry_path.display(), project_name, project_version);
    Ok(())
}


fn help() {
    println!("Oxid 0.3.0");
    println!("Usage:");
    println!("  oxid run <file.ox>");
    println!("  oxid check <file.ox>");
    println!("  oxid repl");
    println!("  oxid new <project-name>");
    println!("  oxid watch <file.ox>");
    println!("  oxid build");
    println!("  oxid help");
}

fn scaffold_project(name: &str) -> Result<(), String> {
    let root = Path::new(name);
    if root.exists() {
        return Err(format!("Already exists: {}", root.display()));
    }
    fs::create_dir_all(root.join("src")).map_err(|e| format!("Failed to create project: {}", e))?;
    fs::create_dir_all(root.join("stdlib")).map_err(|e| format!("Failed to create project: {}", e))?;
    fs::write(
        root.join("src/main.ox"),
        r#"fn main() {
    print "Hello from Oxid";
}
"#,
    )
    .map_err(|e| format!("Failed to create main.ox: {}", e))?;
    fs::write(
        root.join("README.md"),
        r#"# Oxid Project

Generated by `oxid new`.

## Next steps

- Edit `src/main.ox`
- Run `oxid build`
- Run `oxid run src/main.ox`
"#,
    )
    .map_err(|e| format!("Failed to create README.md: {}", e))?;
    fs::write(
        root.join("oxid.toml"),
        r#"[project]
name = "demo"
version = "0.1.0"
entry = "src/main.ox"

[build]
mode = "script-first"
incremental = true
ffi = true

[features]
async = true
macros = true
const_eval = true
c_interop = true
cpp_interop = true
"#,
    )
    .map_err(|e| format!("Failed to create oxid.toml: {}", e))?;
    Ok(())
}

fn main() {
    let mut interp = Interpreter::new();
    let args: Vec<String> = env::args().collect();

    let result = match args.get(1).map(|s| s.as_str()) {
        None => {
            help();
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") => {
            help();
            Ok(())
        }
        Some("repl") => repl(&mut interp),
        Some("check") => {
            let Some(file) = args.get(2) else {
                Err("`oxid check` requires a file path.".to_string())
            };
            match fs::read_to_string(file) {
                Ok(source) => match preprocess_source(&source) {
                    Ok(source) => {
                        let mut parser = Parser::new(&source);
                        match parser.parse_program() {
                            Ok(_) => { println!("syntax ok: {}", file); Ok(()) }
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) => Err(err),
                },
                Err(e) => Err(format!("Cannot read file: {} ({})", file, e)),
            }
        }
        Some("run") => {
            let Some(file) = args.get(2) else {
                Err("`oxid run` requires a file path.".to_string())
            };
            run_file(Path::new(file), &mut interp)
        }
        Some("new") => {
            let Some(name) = args.get(2) else {
                Err("`oxid new` requires a project name.".to_string())
            };
            scaffold_project(name)
        }
        Some("watch") => {
            let Some(file) = args.get(2) else {
                Err("`oxid watch` requires a file path.".to_string())
            };
            watch_file(Path::new(file), &mut interp)
        }
        Some("build") => {
            let cwd = Path::new(".");
            build_project(cwd)
        }
        Some(other) => {
            let path = Path::new(other);
            if path.exists() {
                run_file(path, &mut interp)
            } else {
                Err(format!("Unknown subcommand: {}", other))
            }
        }
    };

    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
