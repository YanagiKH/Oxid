
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::ffi::CString;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    Percent,
    PipeGreater,
    FatArrow,
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
    Invalid(String),
    Fn,
    Async,
    Await,
    Let,
    Const,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Return,
    True,
    False,
    Null,
    Print,
    Use,
    And,
    Or,
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
    Await(Box<Expr>),
    Grouping(Box<Expr>),
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
    For {
        name: String,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Use(String),
}

#[derive(Clone, Debug)]
struct Program {
    stmts: Vec<Stmt>,
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

#[derive(Clone, Debug)]
struct FunctionValue {
    name: String,
    params: Vec<String>,
    body: Vec<Stmt>,
    closure: EnvRef,
    is_async: bool,
}

#[derive(Clone, Debug)]
struct TaskValue {
    function: Rc<FunctionValue>,
    args: Vec<Value>,
}

type EnvRef = Rc<RefCell<Environment>>;

#[derive(Clone, Debug)]
struct Environment {
    values: HashMap<String, Value>,
    enclosing: Option<EnvRef>,
}

#[derive(Debug, Clone)]
enum RuntimeError {
    Message(String),
    Return(Value),
    Break,
    Continue,
}

struct Interpreter {
    globals: EnvRef,
    loaded_modules: HashSet<PathBuf>,
    consts: HashSet<String>,
}

#[derive(Clone, Debug, Default)]
struct MacroDef {
    params: Vec<String>,
    body: String,
}

#[derive(Clone, Debug, Default)]
struct ProjectManifest {
    name: Option<String>,
    version: Option<String>,
    entry: Option<String>,
    scripts: HashMap<String, String>,
    dependencies: HashMap<String, String>,
    features: HashMap<String, bool>,
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "Null"),
            Value::Bool(v) => write!(f, "Bool({v})"),
            Value::Number(v) => write!(f, "Number({v})"),
            Value::String(v) => write!(f, "String({v:?})"),
            Value::Array(v) => write!(f, "Array(len={})", v.borrow().len()),
            Value::Function(v) => write!(f, "Function({})", v.name),
            Value::Task(v) => write!(f, "Task({})", v.function.name),
            Value::NativeFunction(_) => write!(f, "NativeFunction"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Number(v) => {
                if v.fract() == 0.0 { write!(f, "{:.0}", v) } else { write!(f, "{}", v) }
            }
            Value::String(v) => write!(f, "{}", v),
            Value::Array(v) => {
                let rendered = v.borrow().iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                write!(f, "[{}]", rendered)
            }
            Value::Function(v) => write!(f, "<fn {}>", v.name),
            Value::Task(v) => write!(f, "<task {}>", v.function.name),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}

impl Interpreter {
    fn new() -> Self {
        let globals = Rc::new(RefCell::new(Environment::new(None)));
        let mut interp = Self { globals, loaded_modules: HashSet::new(), consts: HashSet::new() };
        interp.install_builtins();
        interp
    }

    fn fork_from_root(root: EnvRef) -> Self {
        Self { globals: root, loaded_modules: HashSet::new(), consts: HashSet::new() }
    }

    fn install_builtins(&mut self) {
        let mut g = self.globals.borrow_mut();
        g.define("clock".into(), Value::NativeFunction(native_clock));
        g.define("now".into(), Value::NativeFunction(native_clock));
        g.define("len".into(), Value::NativeFunction(native_len));
        g.define("push".into(), Value::NativeFunction(native_push));
        g.define("pop".into(), Value::NativeFunction(native_pop));
        g.define("range".into(), Value::NativeFunction(native_range));
        g.define("str".into(), Value::NativeFunction(native_str));
        g.define("spawn".into(), Value::NativeFunction(native_spawn));
        g.define("join".into(), Value::NativeFunction(native_join));
        g.define("join_all".into(), Value::NativeFunction(native_join_all));
        g.define("task_status".into(), Value::NativeFunction(native_task_status));
        g.define("yield_now".into(), Value::NativeFunction(native_yield_now));
        g.define("read_text".into(), Value::NativeFunction(native_read_text));
        g.define("write_text".into(), Value::NativeFunction(native_write_text));
        g.define("exists".into(), Value::NativeFunction(native_exists));
        g.define("env".into(), Value::NativeFunction(native_env));
        g.define("cwd".into(), Value::NativeFunction(native_cwd));
        g.define("list_dir".into(), Value::NativeFunction(native_list_dir));
        g.define("sleep".into(), Value::NativeFunction(native_sleep));
        g.define("sleep_ms".into(), Value::NativeFunction(native_sleep));
        g.define("c_len".into(), Value::NativeFunction(native_c_len));
        g.define("c_hash".into(), Value::NativeFunction(native_c_hash));
        g.define("cpp_len".into(), Value::NativeFunction(native_cpp_len));
        g.define("cpp_hash".into(), Value::NativeFunction(native_cpp_hash));
        g.define("assert".into(), Value::NativeFunction(native_assert));
        g.define("type_of".into(), Value::NativeFunction(native_type_of));
        g.define("number".into(), Value::NativeFunction(native_number));
        g.define("split".into(), Value::NativeFunction(native_split));
        g.define("join_text".into(), Value::NativeFunction(native_join_text));
        g.define("replace".into(), Value::NativeFunction(native_replace));
        g.define("process".into(), Value::NativeFunction(native_process));
        g.define("process_output".into(), Value::NativeFunction(native_process_output));
        g.define("python".into(), Value::NativeFunction(native_python));
        g.define("java".into(), Value::NativeFunction(native_java));
        g.define("go".into(), Value::NativeFunction(native_go));
        g.define("json_escape".into(), Value::NativeFunction(native_json_escape));
        g.define("web_response".into(), Value::NativeFunction(native_web_response));
        g.define("web_serve_once".into(), Value::NativeFunction(native_web_serve_once));
    }

    fn execute_program(&mut self, program: &Program, base_dir: &Path) -> Result<(), String> {
        for stmt in &program.stmts {
            if let Err(err) = self.execute_stmt(stmt, self.globals.clone(), base_dir) {
                return match err {
                    RuntimeError::Message(msg) => Err(msg),
                    RuntimeError::Return(_) => Err("return used outside a function".to_string()),
                    RuntimeError::Break => Err("break used outside a loop".to_string()),
                    RuntimeError::Continue => Err("continue used outside a loop".to_string()),
                };
            }
        }

        if self.has_function("main") {
            let value = self.call_named_function("main", Vec::new())?;
            let value = match value {
                Value::Task(task) => self.execute_task(task)?,
                other => other,
            };
            if !matches!(value, Value::Null) {
                println!("{}", value);
            }
        }
        Ok(())
    }

    fn has_function(&self, name: &str) -> bool {
        matches!(self.globals.borrow().values.get(name), Some(Value::Function(_)) | Some(Value::NativeFunction(_)))
    }

    fn call_named_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let value = self.get_var(name).map_err(|e| e.to_string())?;
        self.call_value(value, args)
    }

    fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, String> {
        match callee {
            Value::Function(func) => {
                if args.len() != func.params.len() {
                    return Err(format!("function `{}` expected {} arguments but received {}", func.name, func.params.len(), args.len()));
                }
                if func.is_async {
                    Ok(Value::Task(Rc::new(TaskValue { function: func.clone(), args })))
                } else {
                    self.invoke_function(func, args)
                }
            }
            Value::Task(task) => self.execute_task(task),
            Value::NativeFunction(f) => f(args).map_err(|e| match e {
                RuntimeError::Message(msg) => msg,
                RuntimeError::Return(_) => "native return".to_string(),
                RuntimeError::Break | RuntimeError::Continue => "native loop control".to_string(),
            }),
            _ => Err("value is not callable".to_string()),
        }
    }

    fn invoke_function(&mut self, func: Rc<FunctionValue>, args: Vec<Value>) -> Result<Value, String> {
        if args.len() != func.params.len() {
            return Err(format!("function `{}` expected {} arguments but received {}", func.name, func.params.len(), args.len()));
        }
        let env = Rc::new(RefCell::new(Environment::new(Some(func.closure.clone()))));
        for (name, value) in func.params.iter().cloned().zip(args) {
            env.borrow_mut().define(name, value);
        }
        match self.execute_block(&func.body, env, Path::new(".")) {
            Ok(()) => Ok(Value::Null),
            Err(RuntimeError::Return(v)) => Ok(v),
            Err(RuntimeError::Message(msg)) => Err(msg),
            Err(RuntimeError::Break) => Err("break used outside a loop".to_string()),
            Err(RuntimeError::Continue) => Err("continue used outside a loop".to_string()),
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
            Stmt::Block(stmts) => self.execute_block(stmts, Rc::new(RefCell::new(Environment::new(Some(env)))), base_dir),
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
                loop {
                    let cond_value = self.evaluate(cond, env.clone(), base_dir).map_err(RuntimeError::Message)?;
                    if !self.is_truthy(&cond_value) {
                        break;
                    }
                    match self.execute_stmt(body, env.clone(), base_dir) {
                        Ok(()) | Err(RuntimeError::Continue) => {}
                        Err(RuntimeError::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(())
            }
            Stmt::For { name, iterable, body } => {
                let iterable = self.evaluate(iterable, env.clone(), base_dir).map_err(RuntimeError::Message)?;
                let values = match iterable {
                    Value::Array(items) => items.borrow().clone(),
                    Value::String(text) => text.chars().map(|ch| Value::String(ch.to_string())).collect(),
                    _ => return Err(RuntimeError::Message("for loops require an array or string".to_string())),
                };
                for value in values {
                    let loop_env = Rc::new(RefCell::new(Environment::new(Some(env.clone()))));
                    loop_env.borrow_mut().define(name.clone(), value);
                    match self.execute_stmt(body, loop_env, base_dir) {
                        Ok(()) | Err(RuntimeError::Continue) => {}
                        Err(RuntimeError::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(())
            }
            Stmt::Function { name, params, body, is_async } => {
                let func = FunctionValue { name: name.clone(), params: params.clone(), body: body.clone(), closure: env.clone(), is_async: *is_async };
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
            Stmt::Break => Err(RuntimeError::Break),
            Stmt::Continue => Err(RuntimeError::Continue),
            Stmt::Use(path) => {
                self.execute_module(path, base_dir).map_err(RuntimeError::Message)?;
                Ok(())
            }
        }
    }

    fn execute_block(&mut self, stmts: &[Stmt], env: EnvRef, base_dir: &Path) -> Result<(), RuntimeError> {
        for stmt in stmts {
            self.execute_stmt(stmt, env.clone(), base_dir)?;
        }
        Ok(())
    }

    fn execute_module(&mut self, path_text: &str, base_dir: &Path) -> Result<(), String> {
        let path = resolve_path(base_dir, path_text);
        let canonical = fs::canonicalize(&path).map_err(|e| format!("cannot open module {}: {}", path.display(), e))?;
        if self.loaded_modules.contains(&canonical) { return Ok(()); }
        self.loaded_modules.insert(canonical.clone());
        let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read module {}: {}", canonical.display(), e))?;
        let source = cached_preprocess(&source, canonical.parent().unwrap_or(base_dir))?;
        let mut parser = Parser::new(&source);
        let program = parser.parse_program()?;
        let parent = canonical.parent().unwrap_or(base_dir);
        for stmt in &program.stmts {
            self.execute_stmt(stmt, self.globals.clone(), parent).map_err(|e| match e {
                RuntimeError::Message(msg) => msg,
                RuntimeError::Return(_) => "return used outside a function".to_string(),
                RuntimeError::Break => "break used outside a loop".to_string(),
                RuntimeError::Continue => "continue used outside a loop".to_string(),
            })?;
        }
        Ok(())
    }

    fn evaluate(&mut self, expr: &Expr, env: EnvRef, base_dir: &Path) -> Result<Value, String> {
        let _evaluation_root = base_dir;
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
            Expr::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.evaluate(item, env.clone(), base_dir)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::Await(inner) => {
                let value = self.evaluate(inner, env, base_dir)?;
                match value {
                    Value::Task(task) => self.execute_task(task),
                    other => Ok(other),
                }
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
                        Value::Number(n) => Ok(Value::Number(0.0 - n)),
                        _ => Err("unary - can only be used with numbers".to_string()),
                    },
                    TokenKind::Bang => Ok(Value::Bool(!self.is_truthy(&right))),
                    _ => Err("unknown unary operator".to_string()),
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
                    TokenKind::Or => if self.is_truthy(&left) { Ok(left) } else { self.evaluate(right, env, base_dir) },
                    TokenKind::And => if !self.is_truthy(&left) { Ok(left) } else { self.evaluate(right, env, base_dir) },
                    _ => Err("unknown logical operator".to_string()),
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
                _ => Err("+ can be used with numbers or strings".to_string()),
            },
            TokenKind::Minus => arithmetic(left, right, |a, b| a - b),
            TokenKind::Star => arithmetic(left, right, |a, b| a * b),
            TokenKind::Slash => match right {
                Value::Number(0.0) => Err("division by zero".to_string()),
                right => arithmetic(left, right, |a, b| a / b),
            },
            TokenKind::Percent => match right {
                Value::Number(0.0) => Err("modulo by zero".to_string()),
                right => arithmetic(left, right, |a, b| a % b),
            },
            TokenKind::Greater => compare(left, right, |a, b| a > b),
            TokenKind::GreaterEqual => compare(left, right, |a, b| a >= b),
            TokenKind::Less => compare(left, right, |a, b| a < b),
            TokenKind::LessEqual => compare(left, right, |a, b| a <= b),
            TokenKind::EqualEqual => Ok(Value::Bool(values_equal(&left, &right))),
            TokenKind::BangEqual => Ok(Value::Bool(!values_equal(&left, &right))),
            _ => Err("unknown binary operator".to_string()),
        }
    }

    fn index_value(&self, target: Value, index: Value) -> Result<Value, String> {
        let idx = as_index(&index)?;
        match target {
            Value::Array(items) => items.borrow().get(idx).cloned().ok_or_else(|| format!("array index {} is out of bounds", idx)),
            Value::String(text) => text.chars().nth(idx).map(|c| Value::String(c.to_string())).ok_or_else(|| format!("string index {} is out of bounds", idx)),
            _ => Err("index access can only be used on arrays or strings".to_string()),
        }
    }

    fn assign_index(&self, target: Value, index: Value, value: Value) -> Result<(), String> {
        let idx = as_index(&index)?;
        match target {
            Value::Array(items) => {
                let mut items = items.borrow_mut();
                if idx >= items.len() { return Err(format!("array index {} is out of bounds", idx)); }
                items[idx] = value;
                Ok(())
            }
            _ => Err("indexed assignment can only be used on arrays".to_string()),
        }
    }

    fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Null => false,
            Value::Bool(v) => *v,
            Value::Number(v) => *v != 0.0,
            Value::String(v) => !v.is_empty(),
            Value::Array(v) => !v.borrow().is_empty(),
            Value::Function(_) | Value::Task(_) | Value::NativeFunction(_) => true,
        }
    }

    fn get_var(&self, name: &str) -> Result<Value, String> {
        self.globals.borrow().values.get(name).cloned().ok_or_else(|| format!("undefined identifier: {}", name))
    }

    fn get_var_scoped(&self, name: &str, env: EnvRef) -> Result<Value, String> {
        Environment::get_scoped(&env, name).ok_or_else(|| format!("undefined identifier: {}", name))
    }

    fn assign_var(&self, name: &str, value: Value, env: EnvRef) -> Result<(), String> {
        if self.consts.contains(name) {
            return Err(format!("cannot reassign constant `{}`", name));
        }
        Environment::assign_scoped(&env, name, value).ok_or_else(|| format!("assignment target not found: {}", name))
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

#[derive(Debug)]
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
        while !self.is_at_end() {
            stmts.push(self.declaration()?);
        }
        Ok(Program { stmts })
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        let is_async = self.match_simple(&[TokenKind::Async]);
        if self.match_simple(&[TokenKind::Fn]) { return self.function_decl(is_async); }
        if self.match_simple(&[TokenKind::Const]) { return self.const_decl(); }
        if self.match_simple(&[TokenKind::Let]) { return self.let_decl(); }
        if self.match_simple(&[TokenKind::Use]) { return self.use_decl(); }
        if is_async { return Err("`async` must be followed by `fn`".to_string()); }
        self.statement()
    }

    fn function_decl(&mut self, is_async: bool) -> Result<Stmt, String> {
        let name = self.consume_identifier("function name is required")?;
        self.consume_simple(TokenKind::LeftParen, "function definition requires `(`")?;
        let mut params = Vec::new();
        if !self.check_simple(&TokenKind::RightParen) {
            loop {
                params.push(self.consume_identifier("parameter name is required")?);
                if !self.match_simple(&[TokenKind::Comma]) { break; }
            }
        }
        self.consume_simple(TokenKind::RightParen, "function definition requires `)`")?;
        if self.match_simple(&[TokenKind::FatArrow, TokenKind::Equal]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "short function must end with `;`")?;
            return Ok(Stmt::Function { name, params, body: vec![Stmt::Return(Some(expr))], is_async });
        }
        self.consume_simple(TokenKind::LeftBrace, "function body requires `{`")?;
        let body = self.block_stmts()?;
        Ok(Stmt::Function { name, params, body, is_async })
    }

    fn const_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("constant name is required")?;
        self.consume_simple(TokenKind::Equal, "constant declaration requires `=`")?;
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "constant declaration must end with `;`")?;
        Ok(Stmt::Const(name, expr))
    }

    fn let_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("variable name is required")?;
        self.consume_simple(TokenKind::Equal, "assignment requires `=`")?;
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "declaration must end with `;`")?;
        Ok(Stmt::Let(name, expr))
    }

    fn use_decl(&mut self) -> Result<Stmt, String> {
        let path = match self.advance().kind.clone() {
            TokenKind::String(s) => s,
            other => return Err(format!("`use` requires a string path, found {:?}", other)),
        };
        self.consume_simple(TokenKind::Semicolon, "`use` must end with `;`")?;
        Ok(Stmt::Use(path))
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.match_simple(&[TokenKind::Print]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`print` must end with `;`")?;
            return Ok(Stmt::Print(expr));
        }
        if self.match_simple(&[TokenKind::Return]) {
            if self.check_simple(&TokenKind::Semicolon) {
                self.advance();
                return Ok(Stmt::Return(None));
            }
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`return` must end with `;`")?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.match_simple(&[TokenKind::Break]) {
            self.consume_simple(TokenKind::Semicolon, "`break` must end with `;`")?;
            return Ok(Stmt::Break);
        }
        if self.match_simple(&[TokenKind::Continue]) {
            self.consume_simple(TokenKind::Semicolon, "`continue` must end with `;`")?;
            return Ok(Stmt::Continue);
        }
        if self.match_simple(&[TokenKind::If]) { return self.if_stmt(); }
        if self.match_simple(&[TokenKind::While]) { return self.while_stmt(); }
        if self.match_simple(&[TokenKind::For]) { return self.for_stmt(); }
        if self.match_simple(&[TokenKind::LeftBrace]) { return Ok(Stmt::Block(self.block_stmts()?)); }
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "expression statements must end with `;`")?;
        Ok(Stmt::Expr(expr))
    }

    fn if_stmt(&mut self) -> Result<Stmt, String> {
        let parenthesized = self.match_simple(&[TokenKind::LeftParen]);
        let cond = self.expression()?;
        if parenthesized { self.consume_simple(TokenKind::RightParen, "`if` condition requires `)`")?; }
        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_simple(&[TokenKind::Else]) { Some(Box::new(self.statement()?)) } else { None };
        Ok(Stmt::If { cond, then_branch, else_branch })
    }

    fn while_stmt(&mut self) -> Result<Stmt, String> {
        let parenthesized = self.match_simple(&[TokenKind::LeftParen]);
        let cond = self.expression()?;
        if parenthesized { self.consume_simple(TokenKind::RightParen, "`while` condition requires `)`")?; }
        let body = Box::new(self.statement()?);
        Ok(Stmt::While { cond, body })
    }

    fn for_stmt(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("`for` requires an item name")?;
        self.consume_simple(TokenKind::In, "`for` requires `in`")?;
        let iterable = self.expression()?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::For { name, iterable, body })
    }

    fn block_stmts(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.check_simple(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.declaration()?);
        }
        self.consume_simple(TokenKind::RightBrace, "block requires `}`")?;
        Ok(stmts)
    }

    fn expression(&mut self) -> Result<Expr, String> { self.assignment() }

    fn assignment(&mut self) -> Result<Expr, String> {
        let expr = self.pipeline()?;
        if self.match_simple(&[TokenKind::Equal]) {
            let value = self.assignment()?;
            return match expr {
                Expr::Variable(name) => Ok(Expr::Assign(name, Box::new(value))),
                Expr::Index(target, index) => Ok(Expr::AssignIndex(target, index, Box::new(value))),
                _ => Err("assignment target must be an identifier or array index".to_string()),
            };
        }
        Ok(expr)
    }

    fn pipeline(&mut self) -> Result<Expr, String> {
        let mut expr = self.or()?;
        while self.match_simple(&[TokenKind::PipeGreater]) {
            let next = self.or()?;
            expr = match next {
                Expr::Call(callee, mut args) => {
                    args.insert(0, expr);
                    Expr::Call(callee, args)
                }
                callee => Expr::Call(Box::new(callee), vec![expr]),
            };
        }
        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut expr = self.and()?;
        while self.match_simple(&[TokenKind::Or]) {
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), TokenKind::Or, Box::new(right));
        }
        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;
        while self.match_simple(&[TokenKind::And]) {
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), TokenKind::And, Box::new(right));
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
        while self.match_simple(&[TokenKind::Greater, TokenKind::GreaterEqual, TokenKind::Less, TokenKind::LessEqual]) {
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
        while self.match_simple(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::Await]) {
            return Ok(Expr::Await(Box::new(self.unary()?)));
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
                        if !self.match_simple(&[TokenKind::Comma]) { break; }
                    }
                }
                self.consume_simple(TokenKind::RightParen, "call requires `)`")?;
                expr = Expr::Call(Box::new(expr), args);
                continue;
            }
            if self.match_simple(&[TokenKind::LeftBracket]) {
                let index = self.expression()?;
                self.consume_simple(TokenKind::RightBracket, "index requires `]`")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::False]) { return Ok(Expr::Literal(Literal::Bool(false))); }
        if self.match_simple(&[TokenKind::True]) { return Ok(Expr::Literal(Literal::Bool(true))); }
        if self.match_simple(&[TokenKind::Null]) { return Ok(Expr::Literal(Literal::Null)); }
        if self.check_number() {
            if let TokenKind::Number(n) = self.advance().kind.clone() { return Ok(Expr::Literal(Literal::Number(n))); }
        }
        if self.check_string() {
            if let TokenKind::String(s) = self.advance().kind.clone() { return Ok(Expr::Literal(Literal::String(s))); }
        }
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() { return Ok(Expr::Variable(name)); }
        }
        if self.match_simple(&[TokenKind::LeftBracket]) {
            let mut items = Vec::new();
            if !self.check_simple(&TokenKind::RightBracket) {
                loop {
                    items.push(self.expression()?);
                    if !self.match_simple(&[TokenKind::Comma]) { break; }
                }
            }
            self.consume_simple(TokenKind::RightBracket, "array requires `]`")?;
            return Ok(Expr::Array(items));
        }
        if self.match_simple(&[TokenKind::LeftParen]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::RightParen, "grouping requires `)`")?;
            return Ok(Expr::Grouping(Box::new(expr)));
        }
        if self.match_simple(&[TokenKind::Eof]) { return Err("unexpected end of file".to_string()); }
        Err(format!("token cannot be parsed as an expression: {:?}", self.peek().kind))
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() { return Ok(name); }
        }
        Err(self.error_here(message))
    }

    fn consume_simple(&mut self, kind: TokenKind, message: &str) -> Result<(), String> {
        if self.check_simple(&kind) { self.advance(); Ok(()) } else { Err(self.error_here(message)) }
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
        matches!((&self.peek().kind, kind),
            (LeftParen, LeftParen)
            | (RightParen, RightParen)
            | (LeftBrace, LeftBrace)
            | (RightBrace, RightBrace)
            | (LeftBracket, LeftBracket)
            | (RightBracket, RightBracket)
            | (Comma, Comma)
            | (Dot, Dot)
            | (Minus, Minus)
            | (Plus, Plus)
            | (Semicolon, Semicolon)
            | (Slash, Slash)
            | (Star, Star)
            | (Percent, Percent)
            | (PipeGreater, PipeGreater)
            | (FatArrow, FatArrow)
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
            | (For, For)
            | (In, In)
            | (Break, Break)
            | (Continue, Continue)
            | (Return, Return)
            | (True, True)
            | (False, False)
            | (Null, Null)
            | (Print, Print)
            | (Use, Use)
            | (And, And)
            | (Or, Or)
            | (Eof, Eof))
    }

    fn check_identifier(&self) -> bool { matches!(self.peek().kind, TokenKind::Identifier(_)) }
    fn check_number(&self) -> bool { matches!(self.peek().kind, TokenKind::Number(_)) }
    fn check_string(&self) -> bool { matches!(self.peek().kind, TokenKind::String(_)) }
    fn is_at_end(&self) -> bool { matches!(self.peek().kind, TokenKind::Eof) }
    fn peek(&self) -> &Token { &self.tokens[self.current] }
    fn previous(&self) -> &Token { &self.tokens[self.current - 1] }
    fn advance(&mut self) -> &Token { if !self.is_at_end() { self.current += 1; } self.previous() }
}

struct Lexer<'a> {
    chars: Vec<char>,
    current: usize,
    line: usize,
    col: usize,
    _source: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { chars: source.chars().collect(), current: 0, line: 1, col: 1, _source: source }
    }

    fn lex(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while !self.is_at_end() {
            if let Some(token) = self.scan_token() { tokens.push(token); }
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
            '%' => Percent,
            '|' => if self.match_char('>') { PipeGreater } else { panic!("`|` must be followed by `>` (line {}, col {})", line, col) },
            '!' => if self.match_char('=') { BangEqual } else { Bang },
            '=' => if self.match_char('=') { EqualEqual } else if self.match_char('>') { FatArrow } else { Equal },
            '<' => if self.match_char('=') { LessEqual } else { Less },
            '>' => if self.match_char('=') { GreaterEqual } else { Greater },
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() { self.advance(); }
                    return None;
                } else if self.match_char('*') {
                    self.block_comment();
                    return None;
                } else {
                    Slash
                }
            }
            '#' => {
                while self.peek() != '\n' && !self.is_at_end() { self.advance(); }
                return None;
            }
            ' ' | '\r' | '\t' => return None,
            '\n' => { self.line += 1; self.col = 1; return None; }
            '"' => return Some(self.string_token(line, col)),
            c if c.is_ascii_digit() => return Some(self.number_token(c, line, col)),
            c if is_alpha(c) => return Some(self.identifier_token(c, line, col)),
            _ => Invalid(format!("unknown character `{}` at line {}, col {}", c, line, col)),
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
            if self.peek() == '\n' { self.line += 1; self.col = 1; }
            self.advance();
        }
    }

    fn string_token(&mut self, line: usize, col: usize) -> Token {
        let mut value = String::new();
        while !self.is_at_end() && self.peek() != '"' {
            let c = self.advance();
            if c == '\\' && !self.is_at_end() {
                let esc = self.advance();
                value.push(match esc { 'n' => '\n', 'r' => '\r', 't' => '\t', '"' => '"', '\\' => '\\', other => other });
            } else {
                if c == '\n' { self.line += 1; self.col = 1; }
                value.push(c);
            }
        }
        if self.is_at_end() {
            Token { kind: TokenKind::Invalid(format!("unterminated string at line {}, col {}", line, col)), line, col }
        } else {
            self.advance();
            Token { kind: TokenKind::String(value), line, col }
        }
    }

    fn number_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while self.peek().is_ascii_digit() { text.push(self.advance()); }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            text.push(self.advance());
            while self.peek().is_ascii_digit() { text.push(self.advance()); }
        }
        Token { kind: TokenKind::Number(text.parse::<f64>().unwrap_or(0.0)), line, col }
    }

    fn identifier_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while is_alpha_numeric(self.peek()) { text.push(self.advance()); }
        let kind = match text.as_str() {
            "fn" | "fun" => TokenKind::Fn,
            "async" | "work" => TokenKind::Async,
            "await" => TokenKind::Await,
            "let" | "var" => TokenKind::Let,
            "const" => TokenKind::Const,
            "if" | "when" => TokenKind::If,
            "else" | "otherwise" => TokenKind::Else,
            "while" | "loop" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" | "give" => TokenKind::Return,
            "true" | "yes" => TokenKind::True,
            "false" | "no" => TokenKind::False,
            "null" | "none" => TokenKind::Null,
            "print" | "say" => TokenKind::Print,
            "use" | "import" => TokenKind::Use,
            "and" | "all" => TokenKind::And,
            "or" | "any" => TokenKind::Or,
            _ => TokenKind::Identifier(text),
        };
        Token { kind, line, col }
    }

    fn is_at_end(&self) -> bool { self.current >= self.chars.len() }
    fn advance(&mut self) -> char { let ch = self.chars[self.current]; self.current += 1; self.col += 1; ch }
    fn match_char(&mut self, expected: char) -> bool { if self.is_at_end() || self.chars[self.current] != expected { return false; } self.current += 1; self.col += 1; true }
    fn peek(&self) -> char { if self.is_at_end() { '\0' } else { self.chars[self.current] } }
    fn peek_next(&self) -> char { if self.current + 1 >= self.chars.len() { '\0' } else { self.chars[self.current + 1] } }
}

fn is_alpha(c: char) -> bool { c.is_ascii_alphabetic() || c == '_' }
fn is_alpha_numeric(c: char) -> bool { is_alpha(c) || c.is_ascii_digit() }

fn root_env(env: &EnvRef) -> EnvRef {
    let mut current = env.clone();
    loop {
        let parent = current.borrow().enclosing.clone();
        match parent {
            Some(next) => current = next,
            None => return current,
        }
    }
}

fn resolve_path(base_dir: &Path, text: &str) -> PathBuf {
    let candidate = Path::new(text);
    if candidate.is_absolute() { return candidate.to_path_buf(); }
    let mut roots = vec![base_dir.to_path_buf(), base_dir.join("src"), base_dir.join("stdlib"), base_dir.join("modules"), base_dir.join("deps"), base_dir.join("vendor")];
    if let Ok(extra) = env::var("OXID_PATH") { for p in env::split_paths(&extra) { roots.push(p); } }
    for root in roots {
        let joined = root.join(candidate);
        if joined.exists() { return joined; }
        if candidate.extension().is_none() {
            let with_ox = joined.with_extension("ox");
            if with_ox.exists() { return with_ox; }
        }
    }
    let fallback = base_dir.join(candidate);
    if candidate.extension().is_none() {
        let with_ox = fallback.with_extension("ox");
        if with_ox.exists() { return with_ox; }
    }
    fallback
}

fn source_fingerprint(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn cache_root(base_dir: &Path) -> PathBuf {
    base_dir.join(".oxid").join("cache")
}

fn preprocess_source(source: &str) -> Result<String, String> {
    let mut macros: HashMap<String, MacroDef> = HashMap::new();
    let mut body_lines = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.starts_with("macro ") {
            let (name, def) = parse_macro_def(line)?;
            macros.insert(name, def);
        } else {
            body_lines.push(raw.to_string());
        }
    }
    let mut out = body_lines.join("\n");
    for _ in 0..8 {
        let (next, changed) = expand_macro_pass(&out, &macros)?;
        out = next;
        if !changed { break; }
    }
    Ok(out)
}

fn parse_macro_def(line: &str) -> Result<(String, MacroDef), String> {
    let rest = line.strip_prefix("macro ").ok_or_else(|| "invalid macro definition".to_string())?;
    let (head, body) = rest.split_once("=>").ok_or_else(|| "macro definition requires =>".to_string())?;
    let head = head.trim();
    let body = body.trim().trim_end_matches(';').trim();
    let open = head.find('(').ok_or_else(|| "macro definition requires (".to_string())?;
    let close = head.rfind(')').ok_or_else(|| "macro definition requires )".to_string())?;
    if close <= open { return Err("invalid macro parameter list".to_string()); }
    let name = head[..open].trim();
    if name.is_empty() { return Err("macro name is empty".to_string()); }
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
    let mut i = 0usize;
    let mut changed = false;
    while i < chars.len() {
        let c = chars[i];
        if is_alpha(c) {
            let start = i;
            i += 1;
            while i < chars.len() && is_alpha_numeric(chars[i]) { i += 1; }
            let ident = chars[start..i].iter().collect::<String>();
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            if let Some(def) = macros.get(&ident) {
                if j < chars.len() && chars[j] == '(' {
                    let (args, end) = parse_macro_args(&chars, j)?;
                    out.push_str(&expand_macro_body(def, &args)?);
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
    let mut current = String::new();
    let mut args = Vec::new();
    let mut i = open_idx + 1;
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
    Err("macro call is missing a closing parenthesis".to_string())
}

fn expand_macro_body(def: &MacroDef, args: &[String]) -> Result<String, String> {
    if args.len() != def.params.len() {
        return Err(format!("macro argument count mismatch: expected {}, got {}", def.params.len(), args.len()));
    }
    let mut body = def.body.clone();
    for (param, arg) in def.params.iter().zip(args.iter()) {
        body = body.replace(param, arg);
    }
    Ok(body)
}

fn cached_preprocess(source: &str, base_dir: &Path) -> Result<String, String> {
    let key = source_fingerprint(source);
    let cache_dir = cache_root(base_dir).join("preprocess");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("cannot create cache directory: {}", e))?;
    let cache_file = cache_dir.join(format!("{}.oxp", key));
    if let Ok(existing) = fs::read_to_string(&cache_file) { return Ok(existing); }
    let processed = preprocess_source(source)?;
    let _ = fs::write(&cache_file, &processed);
    Ok(processed)
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

fn run_source(source: &str, base_dir: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let processed = cached_preprocess(source, base_dir)?;
    let mut parser = Parser::new(&processed);
    let program = parser.parse_program()?;
    interp.execute_program(&program, base_dir)
}

fn run_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open file: {} ({})", path.display(), e))?;
    let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read file: {} ({})", canonical.display(), e))?;
    let base_dir = canonical.parent().unwrap_or(Path::new("."));
    run_source(&source, base_dir, interp)
}

fn run_manifest_script(root: &Path, script_name: &str, extra_args: &[String]) -> Result<(), String> {
    let manifest = load_manifest(&root.join("oxid.toml"))?;
    let script = manifest.scripts.get(script_name).cloned().ok_or_else(|| format!("script `{}` was not found in oxid.toml", script_name))?;
    let command_line = if extra_args.is_empty() { script } else { format!("{} {}", script, extra_args.join(" ")) };
    let status = if cfg!(windows) {
        Command::new("cmd").args(["/C", &command_line]).current_dir(root).status().map_err(|e| format!("failed to launch script: {}", e))?
    } else {
        Command::new("sh").args(["-lc", &command_line]).current_dir(root).status().map_err(|e| format!("failed to launch script: {}", e))?
    };
    if status.success() { Ok(()) } else { Err(format!("script `{}` exited with status {}", script_name, status)) }
}

fn clear_cache(root: &Path) -> Result<(), String> {
    let cache = root.join(".oxid");
    if cache.exists() { fs::remove_dir_all(&cache).map_err(|e| format!("cannot clear cache: {}", e))?; }
    println!("cache cleared: {}", cache.display());
    Ok(())
}

fn format_source(source: &str) -> String {
    let mut out = String::new();
    let mut indent = 0usize;
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if !out.ends_with('\n') { out.push('\n'); }
            continue;
        }
        if line.starts_with('}') { indent = indent.saturating_sub(1); }
        out.push_str(&"    ".repeat(indent));
        out.push_str(line);
        out.push('\n');
        let opens = line.chars().filter(|&c| c == '{').count();
        let closes = line.chars().filter(|&c| c == '}').count();
        if opens > closes { indent += opens - closes; } else { indent = indent.saturating_sub(closes - opens); }
    }
    if !out.ends_with('\n') { out.push('\n'); }
    out
}

fn collect_oxid_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(read_dir) = fs::read_dir(root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if matches!(path.file_name().and_then(|s| s.to_str()), Some(".git" | "target" | ".oxid")) { continue; }
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
                if modified > latest { latest = modified; }
            }
        }
    }
    latest
}

fn watch_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open file: {} ({})", path.display(), e))?;
    let root = canonical.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut tracked = collect_oxid_files(&root);
    if !tracked.contains(&canonical) { tracked.push(canonical.clone()); }
    let mut last = latest_mtime(&tracked);
    println!("watching {} ... (Ctrl+C to stop)", canonical.display());
    loop {
        thread::sleep(Duration::from_millis(500));
        let current = collect_oxid_files(&root);
        let changed = latest_mtime(&current);
        if changed > last {
            last = changed;
            println!("reloading...");
            if let Err(err) = run_file(&canonical, interp) {
                eprintln!("error: {}", err);
            }
        }
    }
}

fn parse_manifest_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches(',').trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

fn load_manifest(path: &Path) -> Result<ProjectManifest, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read manifest: {} ({})", path.display(), e))?;
    let mut manifest = ProjectManifest::default();
    let mut section = String::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_start_matches('[').trim_end_matches(']').to_string();
            continue;
        }
        let Some((key, value_raw)) = line.split_once('=') else { continue; };
        let key = key.trim();
        let value = parse_manifest_value(value_raw).unwrap_or_else(|| value_raw.trim().trim_matches('"').to_string());
        match (section.as_str(), key) {
            ("project", "name") | ("", "name") => manifest.name = Some(value),
            ("project", "version") | ("", "version") => manifest.version = Some(value),
            ("project", "entry") | ("build", "entry") | ("", "entry") => manifest.entry = Some(value),
            ("features", _) => {
                let enabled = matches!(value.as_str(), "true" | "yes" | "on" | "1");
                manifest.features.insert(key.to_string(), enabled);
            }
            ("scripts", _) => { manifest.scripts.insert(key.to_string(), value); }
            ("dependencies", _) => { manifest.dependencies.insert(key.to_string(), value); }
            _ => {}
        }
    }
    Ok(manifest)
}

fn bundle_file(path: &Path, visited: &mut HashSet<PathBuf>) -> Result<String, String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open source {}: {}", path.display(), e))?;
    if !visited.insert(canonical.clone()) { return Ok(String::new()); }
    let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read source {}: {}", canonical.display(), e))?;
    let base_dir = canonical.parent().unwrap_or(Path::new("."));
    let mut bundled = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use \"") {
            if let Some(module_path) = rest.strip_suffix("\";") {
                bundled.push_str(&bundle_file(&resolve_path(base_dir, module_path), visited)?);
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("import \"") {
            if let Some(module_path) = rest.strip_suffix("\";") {
                bundled.push_str(&bundle_file(&resolve_path(base_dir, module_path), visited)?);
                continue;
            }
        }
        bundled.push_str(line);
        bundled.push('\n');
    }
    Ok(bundled)
}

fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    let mut visited = HashSet::new();
    let bundled = bundle_file(input, &mut visited)?;
    let processed = preprocess_source(&bundled)?;
    let mut parser = Parser::new(&processed);
    parser.parse_program()?;
    if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| format!("cannot create output directory: {}", e))?; }
    let artifact = format!("// Oxid bundle v0.8\n{}", processed);
    fs::write(output, artifact).map_err(|e| format!("cannot write bundle {}: {}", output.display(), e))?;
    println!("compiled: {} -> {} ({} modules)", input.display(), output.display(), visited.len());
    Ok(())
}

fn build_project(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("oxid.toml");
    if !manifest_path.exists() { return Err(format!("manifest not found: {}", manifest_path.display())); }
    let manifest = load_manifest(&manifest_path)?;
    for (name, target) in &manifest.dependencies {
        let path = Path::new(target);
        if (target.starts_with("./") || target.starts_with("../") || path.is_absolute()) && !path.exists() {
            return Err(format!("dependency `{}` points to missing path: {}", name, target));
        }
    }
    let entry = manifest.entry.clone().or_else(|| {
        let src_main = root.join("src/main.ox");
        if src_main.exists() { Some("src/main.ox".to_string()) } else { None }
    }).or_else(|| {
        let root_main = root.join("main.ox");
        if root_main.exists() { Some("main.ox".to_string()) } else { None }
    }).ok_or_else(|| "entry file not found. Set `entry` in oxid.toml or create `src/main.ox`".to_string())?;
    let entry_path = root.join(&entry);
    if !entry_path.exists() { return Err(format!("entry file not found: {}", entry_path.display())); }
    let source = fs::read_to_string(&entry_path).map_err(|e| format!("cannot read file: {} ({})", entry_path.display(), e))?;
    let source = cached_preprocess(&source, root)?;
    let mut parser = Parser::new(&source);
    parser.parse_program()?;
    let report = format!(
        "Project: {} {}\nEntry: {}\nFeatures: {}\nScripts: {}\nDependencies: {}\n",
        manifest.name.clone().unwrap_or_else(|| "unknown".to_string()),
        manifest.version.clone().unwrap_or_else(|| "unknown".to_string()),
        entry,
        if manifest.features.is_empty() { "none".to_string() } else { manifest.features.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", ") },
        if manifest.scripts.is_empty() { "none".to_string() } else { manifest.scripts.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", ") },
        if manifest.dependencies.is_empty() { "none".to_string() } else { manifest.dependencies.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", ") },
    );
    let oxid_dir = root.join(".oxid");
    fs::create_dir_all(&oxid_dir).map_err(|e| format!("cannot create build directory: {}", e))?;
    fs::write(oxid_dir.join("build-report.txt"), report).map_err(|e| format!("cannot write build report: {}", e))?;
    let artifact_name = manifest.name.as_deref().unwrap_or("app");
    compile_file(&entry_path, &oxid_dir.join("bin").join(format!("{}.oxb", artifact_name)))?;
    println!("build ok: {}", entry_path.display());
    Ok(())
}

fn format_project(root: &Path) -> Result<(), String> {
    let files = collect_oxid_files(root).into_iter().filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ox")).collect::<Vec<_>>();
    if files.is_empty() { return Err(format!("no Oxid source files found under {}", root.display())); }
    for file in files {
        let source = fs::read_to_string(&file).map_err(|e| format!("cannot read file: {} ({})", file.display(), e))?;
        let formatted = format_source(&source);
        if formatted != source { fs::write(&file, formatted).map_err(|e| format!("cannot write file: {} ({})", file.display(), e))?; }
    }
    Ok(())
}

fn run_test_suite(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    let tests_dir = root.join("tests");
    if tests_dir.exists() { files.extend(collect_oxid_files(&tests_dir).into_iter().filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ox"))); }
    let examples_dir = root.join("examples");
    if examples_dir.exists() { files.extend(collect_oxid_files(&examples_dir).into_iter().filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ox"))); }
    if files.is_empty() { return Err("no test or runnable example Oxid files found".to_string()); }
    files.sort();
    for file in files {
        println!("test: {}", file.display());
        let mut interp = Interpreter::new();
        run_file(&file, &mut interp)?;
    }
    Ok(())
}

fn doctor_project(root: &Path) -> Result<(), String> {
    let checks = [
        ("manifest", root.join("oxid.toml")),
        ("readme-en", root.join("README.md")),
        ("readme-zh", root.join("README_ZH.md")),
        ("readme-jp", root.join("README_JP.md")),
        ("entry", root.join("src/main.ox")),
        ("native", root.join("native")),
        ("docs", root.join("docs")),
        ("examples", root.join("examples")),
        ("stdlib", root.join("stdlib")),
        ("tests", root.join("tests")),
        ("ci", root.join(".github/workflows/ci.yml")),
        ("release", root.join(".github/workflows/release.yml")),
        ("unix-installer", root.join("install.sh")),
        ("windows-installer", root.join("install.ps1")),
    ];
    let mut missing = Vec::new();
    for (name, path) in checks {
        let exists = path.exists();
        println!("{}: {}", name, if exists { "ok" } else { "missing" });
        if !exists { missing.push(name); }
    }
    if missing.is_empty() { Ok(()) } else { Err(format!("project health check failed; missing: {}", missing.join(", "))) }
}

fn document_project(root: &Path) -> Result<(), String> {
    let docs = root.join("docs");
    fs::create_dir_all(&docs).map_err(|e| format!("cannot create docs dir: {}", e))?;
    let api = r#"# Oxid API

## Built-ins

- clock / now
- len / push / pop / range / str
- spawn / join / join_all / task_status / yield_now
- read_text / write_text / exists / env / cwd / list_dir
- sleep / sleep_ms
- assert / type_of
- number / split / join_text / replace / json_escape
- process / process_output / python / java / go
- web_response / web_serve_once
- c_len / c_hash / cpp_len / cpp_hash

## Commands

- oxid run
- oxid script
- oxid repl
- oxid check
- oxid compile
- oxid watch
- oxid build
- oxid clean
- oxid fmt
- oxid test
- oxid doctor
- oxid doc
- oxid new
- oxid init
- oxid add
- oxid bridge
- oxid web new
- oxid discord new

## Language focus

- fast script execution
- ergonomic async tasks
- concise fun / var / say / give / when / for syntax
- single-pass module bundles and pipeline expressions
- macro pre-expansion
- local module loading
- Python, Java, Go, C, and C++ interoperability
- Web routing and Discord interaction modules
"#;
    fs::write(docs.join("API.md"), api).map_err(|e| format!("cannot write docs: {}", e))?;
    Ok(())
}

fn scaffold_project(name: &str) -> Result<(), String> {
    let root = Path::new(name);
    if root.exists() { return Err(format!("already exists: {}", root.display())); }
    fs::create_dir_all(root.join("src")).map_err(|e| format!("failed to create project: {}", e))?;
    fs::create_dir_all(root.join("stdlib")).map_err(|e| format!("failed to create project: {}", e))?;
    fs::create_dir_all(root.join("examples")).map_err(|e| format!("failed to create project: {}", e))?;
    fs::create_dir_all(root.join("tools")).map_err(|e| format!("failed to create project: {}", e))?;
    fs::create_dir_all(root.join("tests")).map_err(|e| format!("failed to create project: {}", e))?;
    fs::write(root.join("src/main.ox"), r#"fun main() {
    say "Hello from Oxid";
}
"#).map_err(|e| format!("failed to create main.ox: {}", e))?;
    fs::write(root.join("examples/hello.ox"), r#"fun repeat_text(text, count) {
    var output = "";
    for item in range(0, count) { output = output + text; }
    give output;
}

fun main() {
    say repeat_text("ox", 3);
}
"#).map_err(|e| format!("failed to create example: {}", e))?;
    fs::write(root.join("stdlib/prelude.ox"), r#"fun ok(value) => value;
fun identity(value) => value;
"#).map_err(|e| format!("failed to create prelude: {}", e))?;
    fs::write(root.join("tools/build.ox"), r#"# Oxid tooling preview
# This file demonstrates how project-level automation can live in Oxid source files.
"#).map_err(|e| format!("failed to create tool file: {}", e))?;
    fs::write(root.join("README.md"), r#"# Oxid Project

Generated by `oxid new`.

## Next steps

- Edit `src/main.ox`
- Run `oxid build`
- Run `oxid run src/main.ox`
- Run `oxid script run`
- Use the standard Oxid modules under `stdlib/`
"#).map_err(|e| format!("failed to create README.md: {}", e))?;
    let manifest = format!(r#"[project]
name = "{}"
version = "0.8.0"
entry = "src/main.ox"

[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
fmt = "oxid fmt"
doc = "oxid doc"
clean = "oxid clean"

[dependencies]

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
java_interop = true
python_interop = true
go_interop = true
web = true
discord = true
"#, name);
    fs::write(root.join("oxid.toml"), manifest).map_err(|e| format!("failed to create oxid.toml: {}", e))?;
    fs::write(root.join("tests/smoke.ox"), r#"fn main() {
    print "smoke";
}
"#).map_err(|e| format!("failed to create smoke test: {}", e))?;
    Ok(())
}

fn scaffold_profile(profile: &str, name: &str) -> Result<(), String> {
    scaffold_project(name)?;
    let root = Path::new(name);
    let source = match profile {
        "web" => r#"fun main() {
    var body = "{\"status\":\"ok\",\"runtime\":\"Oxid\"}";
    var response = web_response(200, "application/json; charset=utf-8", body);
    say "Listening once on http://127.0.0.1:8080";
    web_serve_once("127.0.0.1", 8080, response);
}
"#,
        "discord" => r#"fun command(name, description) => [name, description];

fun main() {
    const token = env("DISCORD_TOKEN");
    when len(token) == 0 {
        say "Set DISCORD_TOKEN before starting the gateway adapter.";
        give none;
    }
    var commands = [command("ping", "Reply with pong"), command("about", "Show bot information")];
    say "Discord command surface ready: " + str(commands);
    say "Use process/process_output or an adapter under bridges/ to connect the Discord gateway.";
}
"#,
        _ => return Err(format!("unknown project profile `{}`; expected web or discord", profile)),
    };
    fs::write(root.join("src/main.ox"), source).map_err(|e| format!("failed to write {} profile: {}", profile, e))?;
    println!("created {} project: {}", profile, root.display());
    Ok(())
}

fn write_bridge_file(root: &Path, relative: &str, content: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| format!("cannot create bridge directory: {}", e))?; }
    fs::write(&path, content).map_err(|e| format!("cannot write bridge {}: {}", path.display(), e))
}

fn scaffold_bridge(target: &str, output: Option<&str>) -> Result<(), String> {
    let root = PathBuf::from(output.unwrap_or(target));
    fs::create_dir_all(&root).map_err(|e| format!("cannot create bridge directory {}: {}", root.display(), e))?;
    match target {
        "python" => write_bridge_file(&root, "oxid_bridge.py", r#"from __future__ import annotations

import subprocess
from pathlib import Path


def run(source: str | Path, *args: object, oxid: str = "oxid") -> str:
    result = subprocess.run(
        [oxid, "run", str(source), *map(str, args)],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.rstrip("\n")
"#)?,
        "java" => write_bridge_file(&root, "OxidBridge.java", r#"import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

public final class OxidBridge {
    private OxidBridge() {}

    public static String run(String source, String... args) throws IOException, InterruptedException {
        List<String> command = new ArrayList<>(List.of("oxid", "run", source));
        command.addAll(List.of(args));
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        int code = process.waitFor();
        if (code != 0) throw new IOException("Oxid exited with " + code + ": " + output);
        return output.stripTrailing();
    }
}
"#)?,
        "go" => write_bridge_file(&root, "oxidbridge/oxidbridge.go", r#"package oxidbridge

import (
	"fmt"
	"os/exec"
	"strings"
)

func Run(source string, args ...string) (string, error) {
	commandArgs := append([]string{"run", source}, args...)
	output, err := exec.Command("oxid", commandArgs...).CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("oxid: %w: %s", err, output)
	}
	return strings.TrimRight(string(output), "\r\n"), nil
}
"#)?,
        "c" => {
            write_bridge_file(&root, "oxid_bridge.h", r#"#ifndef OXID_BRIDGE_H
#define OXID_BRIDGE_H

#include <stddef.h>

int oxid_run(const char *source, char *output, size_t capacity);

#endif
"#)?;
            write_bridge_file(&root, "oxid_bridge.c", r#"#include "oxid_bridge.h"

#include <stdio.h>

#if defined(_WIN32)
#define OXID_POPEN _popen
#define OXID_PCLOSE _pclose
#else
#define OXID_POPEN popen
#define OXID_PCLOSE pclose
#endif

int oxid_run(const char *source, char *output, size_t capacity) {
    char command[4096];
    FILE *pipe;
    size_t used = 0;
    if (!source || !output || capacity == 0) return -1;
    if (snprintf(command, sizeof command, "oxid run \"%s\"", source) >= (int)sizeof command) return -2;
    pipe = OXID_POPEN(command, "r");
    if (!pipe) return -3;
    while (used + 1 < capacity) {
        int ch = fgetc(pipe);
        if (ch == EOF) break;
        output[used++] = (char)ch;
    }
    output[used] = '\0';
    return OXID_PCLOSE(pipe);
}
"#)?;
        }
        "cpp" => write_bridge_file(&root, "oxid_bridge.hpp", r#"#pragma once

#include <array>
#include <cstdio>
#include <stdexcept>
#include <string>

namespace oxid {
inline std::string run(const std::string& source) {
    const std::string command = "oxid run \"" + source + "\"";
#if defined(_WIN32)
    FILE* pipe = _popen(command.c_str(), "r");
#else
    FILE* pipe = popen(command.c_str(), "r");
#endif
    if (!pipe) throw std::runtime_error("failed to start Oxid");
    std::array<char, 4096> buffer{};
    std::string output;
    while (std::fgets(buffer.data(), static_cast<int>(buffer.size()), pipe)) output += buffer.data();
#if defined(_WIN32)
    const int code = _pclose(pipe);
#else
    const int code = pclose(pipe);
#endif
    if (code != 0) throw std::runtime_error("Oxid exited with a failure");
    return output;
}
}
"#)?,
        "all" => {
            for language in ["python", "java", "go", "c", "cpp"] {
                let language_root = root.join(language);
                let language_output = language_root.to_string_lossy().to_string();
                scaffold_bridge(language, Some(&language_output))?;
            }
        }
        _ => return Err(format!("unknown bridge target `{}`; expected python, java, go, c, cpp, or all", target)),
    }
    println!("bridge generated: {} -> {}", target, root.display());
    Ok(())
}

fn add_dependency(root: &Path, name: &str, target: &str) -> Result<(), String> {
    let path = root.join("oxid.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read manifest: {} ({})", path.display(), e))?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    let dep_section = lines.iter().position(|line| line.trim() == "[dependencies]");
    let entry = format!("{} = \"{}\"", name, target);
    if lines.iter().any(|line| line.trim_start().starts_with(&format!("{} =", name))) {
        return Err(format!("dependency `{}` already exists", name));
    }
    if let Some(dep_idx) = dep_section {
        let mut insert_at = lines.len();
        for (idx, line) in lines.iter().enumerate().skip(dep_idx + 1) {
            if line.starts_with('[') {
                insert_at = idx;
                break;
            }
        }
        lines.insert(insert_at, entry);
    } else {
        lines.push(String::new());
        lines.push(String::from("[dependencies]"));
        lines.push(entry);
    }
    fs::write(&path, lines.join("\n") + "\n").map_err(|e| format!("cannot write manifest: {} ({})", path.display(), e))?;
    Ok(())
}

fn native_clock(_: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| RuntimeError::Message(format!("failed to get clock: {}", e)))?;
    Ok(Value::Number(now.as_secs_f64()))
}

fn native_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("len requires 1 argument".to_string())); }
    match &args[0] {
        Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
        Value::Array(items) => Ok(Value::Number(items.borrow().len() as f64)),
        _ => Err(RuntimeError::Message("len can only be used with strings or arrays".to_string())),
    }
}

fn native_push(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("push requires 2 arguments".to_string())); }
    match &args[0] {
        Value::Array(items) => { items.borrow_mut().push(args[1].clone()); Ok(Value::Null) }
        _ => Err(RuntimeError::Message("push requires an array".to_string())),
    }
}

fn native_pop(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("pop requires 1 argument".to_string())); }
    match &args[0] {
        Value::Array(items) => Ok(items.borrow_mut().pop().unwrap_or(Value::Null)),
        _ => Err(RuntimeError::Message("pop requires an array".to_string())),
    }
}

fn native_range(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("range requires 2 arguments".to_string())); }
    let start = as_index(&args[0]).map_err(RuntimeError::Message)?;
    let end = as_index(&args[1]).map_err(RuntimeError::Message)?;
    let values = (start..end).map(|n| Value::Number(n as f64)).collect::<Vec<_>>();
    Ok(Value::Array(Rc::new(RefCell::new(values))))
}

fn native_str(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("str requires 1 argument".to_string())); }
    Ok(Value::String(args[0].to_string()))
}

fn native_spawn(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() { return Err(RuntimeError::Message("spawn requires at least 1 argument".to_string())); }
    match &args[0] {
        Value::Function(func) => {
            let extra = args[1..].to_vec();
            if extra.len() != func.params.len() {
                return Err(RuntimeError::Message(format!("spawn arity mismatch for `{}`", func.name)));
            }
            Ok(Value::Task(Rc::new(TaskValue { function: func.clone(), args: extra })))
        }
        Value::Task(task) => Ok(Value::Task(task.clone())),
        _ => Err(RuntimeError::Message("spawn requires a function or task".to_string())),
    }
}

fn native_join(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("join requires 1 task argument".to_string())); }
    match &args[0] {
        Value::Task(task) => {
            let root = root_env(&task.function.closure);
            let mut interp = Interpreter::fork_from_root(root);
            interp.execute_task(task.clone()).map_err(RuntimeError::Message)
        }
        Value::Function(func) => {
            if !func.params.is_empty() {
                return Err(RuntimeError::Message(format!("join requires a task or a zero-argument function, got `{}`", func.name)));
            }
            let root = root_env(&func.closure);
            let mut interp = Interpreter::fork_from_root(root);
            let task = Rc::new(TaskValue { function: func.clone(), args: Vec::new() });
            interp.execute_task(task).map_err(RuntimeError::Message)
        }
        _ => Err(RuntimeError::Message("join requires a task or function value".to_string())),
    }
}

fn native_join_all(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("join_all requires 1 array argument".to_string())); }
    let items = match &args[0] {
        Value::Array(items) => items.borrow().clone(),
        _ => return Err(RuntimeError::Message("join_all requires an array of tasks or functions".to_string())),
    };
    let root = items.iter().find_map(|item| match item {
        Value::Task(task) => Some(root_env(&task.function.closure)),
        Value::Function(func) => Some(root_env(&func.closure)),
        _ => None,
    }).unwrap_or_else(|| Rc::new(RefCell::new(Environment::new(None))));
    let mut interp = Interpreter::fork_from_root(root);
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let value = match item {
            Value::Task(task) => interp.execute_task(task).map_err(RuntimeError::Message)?,
            Value::Function(func) => interp.execute_task(Rc::new(TaskValue { function: func, args: Vec::new() })).map_err(RuntimeError::Message)?,
            _ => return Err(RuntimeError::Message("join_all accepts only tasks or functions".to_string())),
        };
        results.push(value);
    }
    Ok(Value::Array(Rc::new(RefCell::new(results))))
}

fn native_task_status(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("task_status requires 1 argument".to_string())); }
    match &args[0] {
        Value::Task(_) => Ok(Value::String("pending".to_string())),
        Value::Function(_) => Ok(Value::String("ready".to_string())),
        _ => Ok(Value::String("not-a-task".to_string())),
    }
}

fn native_yield_now(_: Vec<Value>) -> Result<Value, RuntimeError> {
    thread::yield_now();
    Ok(Value::Null)
}

fn native_read_text(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("read_text requires 1 path argument".to_string())); }
    let path = match &args[0] {
        Value::String(s) => s,
        _ => return Err(RuntimeError::Message("read_text requires a string path".to_string())),
    };
    let text = fs::read_to_string(path).map_err(|e| RuntimeError::Message(format!("failed to read {}: {}", path, e)))?;
    Ok(Value::String(text))
}

fn native_write_text(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("write_text requires 2 arguments".to_string())); }
    let path = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Err(RuntimeError::Message("write_text requires a string path".to_string())),
    };
    let text = match &args[1] {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    fs::write(&path, text).map_err(|e| RuntimeError::Message(format!("failed to write {}: {}", path, e)))?;
    Ok(Value::Null)
}

fn native_exists(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("exists requires 1 path argument".to_string())); }
    let path = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("exists requires a string path".to_string())), };
    Ok(Value::Bool(Path::new(path).exists()))
}

fn native_env(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("env requires 1 key argument".to_string())); }
    let key = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("env requires a string key".to_string())), };
    Ok(Value::String(env::var(key).unwrap_or_default()))
}

fn native_cwd(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() { return Err(RuntimeError::Message("cwd takes no arguments".to_string())); }
    let cwd = env::current_dir().map_err(|e| RuntimeError::Message(format!("failed to get current directory: {}", e)))?;
    Ok(Value::String(cwd.display().to_string()))
}

fn native_list_dir(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("list_dir requires 1 path argument".to_string())); }
    let path = match &args[0] { Value::String(s) => s.clone(), _ => return Err(RuntimeError::Message("list_dir requires a string path".to_string())), };
    let mut items = Vec::new();
    for entry in fs::read_dir(&path).map_err(|e| RuntimeError::Message(format!("failed to list {}: {}", path, e)))? {
        let entry = entry.map_err(|e| RuntimeError::Message(format!("failed to read directory entry: {}", e)))?;
        items.push(Value::String(entry.path().display().to_string()));
    }
    Ok(Value::Array(Rc::new(RefCell::new(items))))
}

fn native_sleep(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("sleep requires 1 argument".to_string())); }
    let ms = as_index(&args[0]).map_err(RuntimeError::Message)? as u64;
    thread::sleep(Duration::from_millis(ms));
    Ok(Value::Null)
}

fn native_c_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("c_len requires 1 string argument".to_string())); }
    let s = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("c_len requires a string argument".to_string())), };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("string contains a NUL byte".to_string()))?;
    let len = unsafe { oxid_c_strlen(cstr.as_ptr()) };
    Ok(Value::Number(len as f64))
}

fn native_c_hash(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("c_hash requires 1 string argument".to_string())); }
    let s = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("c_hash requires a string argument".to_string())), };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("string contains a NUL byte".to_string()))?;
    let hash = unsafe { oxid_c_hash(cstr.as_ptr()) };
    Ok(Value::String(format!("{:016x}", hash)))
}

fn native_cpp_len(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("cpp_len requires 1 string argument".to_string())); }
    let s = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("cpp_len requires a string argument".to_string())), };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("string contains a NUL byte".to_string()))?;
    let len = unsafe { oxid_cpp_len(cstr.as_ptr()) };
    Ok(Value::Number(len as f64))
}

fn native_cpp_hash(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("cpp_hash requires 1 string argument".to_string())); }
    let s = match &args[0] { Value::String(s) => s, _ => return Err(RuntimeError::Message("cpp_hash requires a string argument".to_string())), };
    let cstr = CString::new(s.as_str()).map_err(|_| RuntimeError::Message("string contains a NUL byte".to_string()))?;
    let hash = unsafe { oxid_cpp_hash(cstr.as_ptr()) };
    Ok(Value::String(format!("{:016x}", hash)))
}

fn native_assert(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() { return Err(RuntimeError::Message("assert requires at least 1 argument".to_string())); }
    if args.len() == 1 {
        if truthy(&args[0]) { Ok(Value::Null) } else { Err(RuntimeError::Message("assert failed".to_string())) }
    } else if truthy(&args[0]) {
        Ok(Value::Null)
    } else {
        Err(RuntimeError::Message(args[1].to_string()))
    }
}

fn native_type_of(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("type_of requires 1 argument".to_string())); }
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

fn native_number(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("number requires 1 argument".to_string())); }
    match &args[0] {
        Value::Number(value) => Ok(Value::Number(*value)),
        Value::String(value) => value.parse::<f64>().map(Value::Number).map_err(|_| RuntimeError::Message(format!("cannot convert `{}` to number", value))),
        Value::Bool(value) => Ok(Value::Number(if *value { 1.0 } else { 0.0 })),
        _ => Err(RuntimeError::Message("number accepts a number, string, or bool".to_string())),
    }
}

fn native_split(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("split requires text and separator".to_string())); }
    let text = value_string_arg(&args[0], "split text")?;
    let separator = value_string_arg(&args[1], "split separator")?;
    let parts = if separator.is_empty() {
        text.chars().map(|ch| Value::String(ch.to_string())).collect()
    } else {
        text.split(&separator).map(|part| Value::String(part.to_string())).collect()
    };
    Ok(Value::Array(Rc::new(RefCell::new(parts))))
}

fn native_join_text(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("join_text requires an array and separator".to_string())); }
    let items = value_array_arg(&args[0], "join_text values")?;
    let separator = value_string_arg(&args[1], "join_text separator")?;
    Ok(Value::String(items.iter().map(ToString::to_string).collect::<Vec<_>>().join(&separator)))
}

fn native_replace(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 { return Err(RuntimeError::Message("replace requires text, pattern, and replacement".to_string())); }
    let text = value_string_arg(&args[0], "replace text")?;
    let pattern = value_string_arg(&args[1], "replace pattern")?;
    let replacement = value_string_arg(&args[2], "replace replacement")?;
    Ok(Value::String(text.replace(&pattern, &replacement)))
}

fn value_string_arg(value: &Value, label: &str) -> Result<String, RuntimeError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        _ => Err(RuntimeError::Message(format!("{} must be a string", label))),
    }
}

fn value_array_arg(value: &Value, label: &str) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Array(values) => Ok(values.borrow().clone()),
        _ => Err(RuntimeError::Message(format!("{} must be an array", label))),
    }
}

fn command_from_values(program: &Value, args: &Value) -> Result<Command, RuntimeError> {
    let program = value_string_arg(program, "program")?;
    let args = value_array_arg(args, "process arguments")?;
    let mut command = Command::new(program);
    for arg in args { command.arg(arg.to_string()); }
    Ok(command)
}

fn native_process(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("process requires a program and argument array".to_string())); }
    let status = command_from_values(&args[0], &args[1])?.status().map_err(|e| RuntimeError::Message(format!("failed to launch process: {}", e)))?;
    Ok(Value::Number(status.code().unwrap_or(1) as f64))
}

fn native_process_output(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 { return Err(RuntimeError::Message("process_output requires a program and argument array".to_string())); }
    let output = command_from_values(&args[0], &args[1])?.output().map_err(|e| RuntimeError::Message(format!("failed to launch process: {}", e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RuntimeError::Message(format!("process exited with {}{}", output.status, if stderr.is_empty() { String::new() } else { format!(": {}", stderr) })));
    }
    Ok(Value::String(String::from_utf8_lossy(&output.stdout).trim_end().to_string()))
}

fn run_language(program: &str, prefix: &[&str], args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() || args.len() > 2 { return Err(RuntimeError::Message(format!("{} bridge requires a target and optional argument array", program))); }
    let target = value_string_arg(&args[0], "bridge target")?;
    let mut process_args = prefix.iter().map(|value| Value::String((*value).to_string())).collect::<Vec<_>>();
    process_args.push(Value::String(target));
    if let Some(extra) = args.get(1) { process_args.extend(value_array_arg(extra, "bridge arguments")?); }
    native_process_output(vec![Value::String(program.to_string()), Value::Array(Rc::new(RefCell::new(process_args)))])
}

fn native_python(args: Vec<Value>) -> Result<Value, RuntimeError> {
    run_language(if cfg!(windows) { "python" } else { "python3" }, &[], args)
}

fn native_java(args: Vec<Value>) -> Result<Value, RuntimeError> { run_language("java", &[], args) }

fn native_go(args: Vec<Value>) -> Result<Value, RuntimeError> { run_language("go", &["run"], args) }

fn native_json_escape(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("json_escape requires 1 string".to_string())); }
    let text = value_string_arg(&args[0], "json_escape value")?;
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for ch in text.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    Ok(Value::String(escaped))
}

fn native_web_response(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 { return Err(RuntimeError::Message("web_response requires status, content type, and body".to_string())); }
    let status = as_index(&args[0]).map_err(RuntimeError::Message)?;
    let content_type = value_string_arg(&args[1], "content type")?;
    let body = value_string_arg(&args[2], "response body")?;
    let reason = match status { 200 => "OK", 201 => "Created", 204 => "No Content", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden", 404 => "Not Found", 500 => "Internal Server Error", _ => "Response" };
    Ok(Value::String(format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, reason, content_type, body.len(), body)))
}

fn native_web_serve_once(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 { return Err(RuntimeError::Message("web_serve_once requires host, port, and response".to_string())); }
    let host = value_string_arg(&args[0], "web host")?;
    let port = as_index(&args[1]).map_err(RuntimeError::Message)?;
    if port > u16::MAX as usize { return Err(RuntimeError::Message("web port must be between 0 and 65535".to_string())); }
    let response = value_string_arg(&args[2], "web response")?;
    let listener = TcpListener::bind((host.as_str(), port as u16)).map_err(|e| RuntimeError::Message(format!("cannot bind web listener: {}", e)))?;
    let (mut stream, _) = listener.accept().map_err(|e| RuntimeError::Message(format!("cannot accept web request: {}", e)))?;
    let mut request = [0u8; 8192];
    let _ = stream.read(&mut request);
    stream.write_all(response.as_bytes()).map_err(|e| RuntimeError::Message(format!("cannot write web response: {}", e)))?;
    stream.flush().map_err(|e| RuntimeError::Message(format!("cannot flush web response: {}", e)))?;
    Ok(Value::Null)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => *v != 0.0,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.borrow().is_empty(),
        Value::Function(_) | Value::Task(_) | Value::NativeFunction(_) => true,
    }
}

fn as_index(value: &Value) -> Result<usize, String> {
    match value {
        Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
        _ => Err("index must be a non-negative integer".to_string()),
    }
}

fn arithmetic(left: Value, right: Value, op: fn(f64, f64) -> f64) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(op(a, b))),
        _ => Err("arithmetic operations require numbers".to_string()),
    }
}

fn compare(left: Value, right: Value, op: fn(f64, f64) -> bool) -> Result<Value, String> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err("comparison operations require numbers".to_string()),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
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

extern "C" {
    fn oxid_c_strlen(s: *const c_char) -> usize;
    fn oxid_c_hash(s: *const c_char) -> u64;
    fn oxid_cpp_len(s: *const c_char) -> usize;
    fn oxid_cpp_hash(s: *const c_char) -> u64;
}

fn help() {
    println!("Oxid 0.8.0");
    println!("Usage:");
    println!("  oxid run <file.ox>");
    println!("  oxid script <name> [args...]");
    println!("  oxid check <file.ox>");
    println!("  oxid compile <file.ox> [-o app.oxb]");
    println!("  oxid repl");
    println!("  oxid new <project-name>");
    println!("  oxid init <project-name>");
    println!("  oxid add <name> <path-or-target>");
    println!("  oxid bridge <python|java|go|c|cpp|all> [output]");
    println!("  oxid web new <project-name>");
    println!("  oxid discord new <project-name>");
    println!("  oxid watch <file.ox>");
    println!("  oxid build");
    println!("  oxid clean");
    println!("  oxid fmt [path]");
    println!("  oxid test");
    println!("  oxid doctor");
    println!("  oxid doc");
    println!("  oxid help");
}

fn main() {
    let mut interp = Interpreter::new();
    let args: Vec<String> = env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        None | Some("help") | Some("--help") | Some("-h") => { help(); Ok(()) }
        Some("run") => match args.get(2) {
            Some(file) => run_file(Path::new(file), &mut interp),
            None => Err("`oxid run` requires a file path".to_string()),
        },
        Some("script") => match args.get(2) {
            Some(name) => run_manifest_script(Path::new("."), name, &args[3..]),
            None => Err("`oxid script` requires a script name".to_string()),
        },
        Some("check") => match args.get(2) {
            Some(file) => {
                let path = Path::new(file);
                let source = fs::read_to_string(path).map_err(|e| format!("cannot read file: {} ({})", path.display(), e));
                match source {
                    Ok(source) => {
                        let base_dir = path.parent().unwrap_or(Path::new("."));
                        match cached_preprocess(&source, base_dir) {
                            Ok(source) => {
                                let mut parser = Parser::new(&source);
                                parser.parse_program().map(|_| { println!("syntax ok: {}", file); })
                            }
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            None => Err("`oxid check` requires a file path".to_string()),
        },
        Some("compile") => match args.get(2) {
            Some(file) => {
                let input = Path::new(file);
                if args.get(3).map(String::as_str) == Some("-o") && args.get(4).is_none() {
                    Err("`-o` requires an output path".to_string())
                } else {
                    let output = if args.get(3).map(String::as_str) == Some("-o") {
                        PathBuf::from(&args[4])
                    } else {
                        input.with_extension("oxb")
                    };
                    compile_file(input, &output)
                }
            }
            None => Err("`oxid compile` requires a source file".to_string()),
        },
        Some("repl") => repl(&mut interp),
        Some("new") => match args.get(2) { Some(name) => scaffold_project(name), None => Err("`oxid new` requires a project name".to_string()) },
        Some("init") => match args.get(2) { Some(name) => scaffold_project(name), None => Err("`oxid init` requires a project name".to_string()) },
        Some("add") => match (args.get(2), args.get(3)) {
            (Some(name), Some(target)) => add_dependency(Path::new("."), name, target),
            _ => Err("`oxid add` requires a dependency name and target".to_string()),
        },
        Some("bridge") => match args.get(2) {
            Some(target) => scaffold_bridge(target, args.get(3).map(String::as_str)),
            None => Err("`oxid bridge` requires python, java, go, c, cpp, or all".to_string()),
        },
        Some("web") => match (args.get(2).map(String::as_str), args.get(3)) {
            (Some("new"), Some(name)) => scaffold_profile("web", name),
            _ => Err("usage: oxid web new <project-name>".to_string()),
        },
        Some("discord") => match (args.get(2).map(String::as_str), args.get(3)) {
            (Some("new"), Some(name)) => scaffold_profile("discord", name),
            _ => Err("usage: oxid discord new <project-name>".to_string()),
        },
        Some("watch") => match args.get(2) { Some(file) => watch_file(Path::new(file), &mut interp), None => Err("`oxid watch` requires a file path".to_string()) },
        Some("build") => build_project(Path::new(".")),
        Some("clean") => clear_cache(Path::new(".")),
        Some("fmt") => {
            let target = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            if target.is_dir() { format_project(&target) } else {
                match fs::read_to_string(&target) {
                    Ok(source) => fs::write(&target, format_source(&source)).map_err(|e| format!("cannot write file: {} ({})", target.display(), e)),
                    Err(e) => Err(format!("cannot read file: {} ({})", target.display(), e)),
                }
            }
        }
        Some("test") => run_test_suite(Path::new(".")),
        Some("doctor") => doctor_project(Path::new(".")),
        Some("doc") => document_project(Path::new(".")),
        Some(other) => Err(format!("unknown subcommand: {}", other)),
    };
    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn evaluate_global(source: &str, name: &str) -> Value {
        let mut interpreter = Interpreter::new();
        run_source(source, Path::new("."), &mut interpreter).expect("source should run");
        interpreter.get_var(name).expect("global should exist")
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        env::temp_dir().join(format!("oxid-{}-{}-{}", label, std::process::id(), nonce))
    }

    #[test]
    fn shortcut_keywords_and_pipeline_execute() {
        let value = evaluate_global(
            "fun double(value) => value * 2;\nconst result = 5 |> double |> str;",
            "result",
        );
        assert!(matches!(value, Value::String(ref text) if text == "10"));
    }

    #[test]
    fn for_break_continue_and_modulo_execute() {
        let value = evaluate_global(
            r#"
fun total() {
    var result = 0;
    for value in range(0, 10) {
        when value == 8 { break; }
        when value % 2 == 0 { continue; }
        result = result + value;
    }
    give result;
}
const answer = total();
"#,
            "answer",
        );
        assert!(matches!(value, Value::Number(number) if number == 16.0));
    }

    #[test]
    fn aliases_and_optional_parentheses_parse() {
        let source = r#"
fun choose(flag) {
    when flag { give yes; } otherwise { give no; }
}
const answer = choose(true);
"#;
        let value = evaluate_global(source, "answer");
        assert!(matches!(value, Value::Bool(true)));
    }

    #[test]
    fn bundle_inlines_modules_and_runs() {
        let root = unique_temp_dir("bundle");
        fs::create_dir_all(&root).expect("temp project");
        fs::write(root.join("lib.ox"), "fun twice(value) => value * 2;\n").expect("module");
        fs::write(root.join("main.ox"), "import \"lib.ox\";\nconst result = twice(21);\n").expect("entry");
        let output = root.join("app.oxb");
        compile_file(&root.join("main.ox"), &output).expect("compile bundle");
        let bundle = fs::read_to_string(&output).expect("bundle output");
        assert!(bundle.contains("fun twice"));
        assert!(!bundle.contains("import \"lib.ox\""));
        let mut interpreter = Interpreter::new();
        run_file(&output, &mut interpreter).expect("run bundle");
        assert!(matches!(interpreter.get_var("result"), Ok(Value::Number(number)) if number == 42.0));
        fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn all_bridge_templates_are_generated() {
        let root = unique_temp_dir("bridges");
        let output = root.to_string_lossy().to_string();
        scaffold_bridge("all", Some(&output)).expect("generate bridges");
        for path in [
            "python/oxid_bridge.py",
            "java/OxidBridge.java",
            "go/oxidbridge/oxidbridge.go",
            "c/oxid_bridge.c",
            "cpp/oxid_bridge.hpp",
        ] {
            assert!(root.join(path).is_file(), "missing {path}");
        }
        fs::remove_dir_all(&root).expect("remove bridge temp project");
    }

    #[test]
    fn json_and_web_helpers_render_valid_shapes() {
        let escaped = native_json_escape(vec![Value::String("a\n\"b".to_string())]).expect("json escape");
        assert!(matches!(escaped, Value::String(ref text) if text == "\"a\\n\\\"b\""));
        let response = native_web_response(vec![
            Value::Number(200.0),
            Value::String("text/plain".to_string()),
            Value::String("ok".to_string()),
        ]).expect("web response");
        assert!(matches!(response, Value::String(ref text) if text.contains("HTTP/1.1 200 OK") && text.ends_with("\r\n\r\nok")));
    }

    #[test]
    fn native_c_and_cpp_bridges_are_linked() {
        assert!(matches!(native_c_len(vec![Value::String("oxid".to_string())]), Ok(Value::Number(4.0))));
        assert!(matches!(native_cpp_len(vec![Value::String("bridge".to_string())]), Ok(Value::Number(6.0))));
    }

    #[test]
    fn invalid_source_and_zero_division_return_errors() {
        let mut parser = Parser::new("fun main() { say @; }");
        assert!(parser.parse_program().is_err());
        let mut interpreter = Interpreter::new();
        assert!(run_source("const value = 1 / 0;", Path::new("."), &mut interpreter).is_err());
    }
}
