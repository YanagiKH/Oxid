
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::ffi::CString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, SystemTime};

#[path = "runtime/artifact.rs"]
mod artifact;
#[path = "runtime/data.rs"]
mod data;
#[path = "runtime/network.rs"]
mod network;
#[path = "runtime/packages.rs"]
mod packages;
#[path = "runtime/benchmark.rs"]
mod benchmark;

#[path = "legacy/syntax.rs"]
mod legacy_syntax;
use legacy_syntax::{
    is_alpha, is_alpha_numeric, Expr, Literal, Parser, Program, SourceSpan, Stmt, TokenKind,
};

const OXID_VERSION: &str = "0.9.0";

#[derive(Clone)]
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Record(Rc<RefCell<BTreeMap<String, Value>>>),
    Function(Rc<FunctionValue>),
    Task(Rc<TaskValue>),
    Listener(Rc<network::ListenerHandle>),
    Connection(Rc<network::ConnectionHandle>),
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
    source_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct TaskValue {
    function: Rc<FunctionValue>,
    args: Vec<Value>,
    state: RefCell<TaskState>,
}

#[derive(Clone, Debug)]
enum TaskState {
    Pending,
    Running,
    Completed(Result<Value, String>),
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
    Located(SourceSpan, String),
    Return(Value),
    Break,
    Continue,
}

fn runtime_error_to_string(error: RuntimeError) -> String {
    match error {
        RuntimeError::Message(message) => message,
        RuntimeError::Located(span, message) => format!("{}: {}", span.render(), message),
        RuntimeError::Return(_) => "return used outside a function".to_string(),
        RuntimeError::Break => "break used outside a loop".to_string(),
        RuntimeError::Continue => "continue used outside a loop".to_string(),
    }
}

struct Interpreter {
    globals: EnvRef,
    loading_modules: HashSet<PathBuf>,
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
            Value::Record(v) => write!(f, "Record(len={})", v.borrow().len()),
            Value::Function(v) => write!(f, "Function({})", v.name),
            Value::Task(v) => write!(f, "Task({})", v.function.name),
            Value::Listener(v) => write!(f, "Listener({})", v.local_addr()),
            Value::Connection(v) => write!(f, "Connection({})", v.peer_addr()),
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
            Value::Record(v) => {
                let rendered = v
                    .borrow()
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{{{}}}", rendered)
            }
            Value::Function(v) => write!(f, "<fn {}>", v.name),
            Value::Task(v) => write!(f, "<task {}>", v.function.name),
            Value::Listener(v) => write!(f, "<listener {}>", v.local_addr()),
            Value::Connection(v) => write!(f, "<connection {}>", v.peer_addr()),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}

impl Interpreter {
    fn new() -> Self {
        let globals = Rc::new(RefCell::new(Environment::new(None)));
        let mut interp = Self {
            globals,
            loading_modules: HashSet::new(),
            loaded_modules: HashSet::new(),
            consts: HashSet::new(),
        };
        interp.install_builtins();
        interp
    }

    fn fork_from_root(root: EnvRef) -> Self {
        Self {
            globals: root,
            loading_modules: HashSet::new(),
            loaded_modules: HashSet::new(),
            consts: HashSet::new(),
        }
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
        g.define("json_parse".into(), Value::NativeFunction(data::json_parse));
        g.define("json_stringify".into(), Value::NativeFunction(data::json_stringify));
        g.define("keys".into(), Value::NativeFunction(data::keys));
        g.define("has_key".into(), Value::NativeFunction(data::has_key));
        g.define("get".into(), Value::NativeFunction(data::get));
        g.define("set".into(), Value::NativeFunction(data::set));
        g.define("remove".into(), Value::NativeFunction(data::remove));
        g.define("record".into(), Value::NativeFunction(data::record));
        g.define("net_listen".into(), Value::NativeFunction(network::net_listen));
        g.define("net_local_addr".into(), Value::NativeFunction(network::net_local_addr));
        g.define("net_accept".into(), Value::NativeFunction(network::net_accept));
        g.define("net_try_accept".into(), Value::NativeFunction(network::net_try_accept));
        g.define("net_read".into(), Value::NativeFunction(network::net_read));
        g.define("net_write".into(), Value::NativeFunction(network::net_write));
        g.define("net_close".into(), Value::NativeFunction(network::net_close));
        g.define("http_read_request".into(), Value::NativeFunction(network::http_read_request));
        g.define("http_write_response".into(), Value::NativeFunction(network::http_write_response));
        g.define("web_response".into(), Value::NativeFunction(native_web_response));
        g.define("web_serve_once".into(), Value::NativeFunction(native_web_serve_once));
    }

    fn execute_program(&mut self, program: &Program, base_dir: &Path) -> Result<(), String> {
        for stmt in &program.stmts {
            if let Err(err) = self.execute_stmt(stmt, self.globals.clone(), base_dir) {
                return Err(runtime_error_to_string(err));
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
                    Ok(Value::Task(Rc::new(TaskValue {
                        function: func.clone(),
                        args,
                        state: RefCell::new(TaskState::Pending),
                    })))
                } else {
                    self.invoke_function(func, args)
                }
            }
            Value::Task(task) => self.execute_task(task),
            Value::NativeFunction(f) => f(args).map_err(|e| match e {
                RuntimeError::Message(msg) => msg,
                RuntimeError::Located(span, msg) => format!("{}: {}", span.render(), msg),
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
        match self.execute_block(&func.body, env, &func.source_dir) {
            Ok(()) => Ok(Value::Null),
            Err(RuntimeError::Return(v)) => Ok(v),
            Err(RuntimeError::Message(msg)) => Err(msg),
            Err(RuntimeError::Located(span, msg)) => Err(format!("{}: {}", span.render(), msg)),
            Err(RuntimeError::Break) => Err("break used outside a loop".to_string()),
            Err(RuntimeError::Continue) => Err("continue used outside a loop".to_string()),
        }
    }

    fn execute_task(&mut self, task: Rc<TaskValue>) -> Result<Value, String> {
        match &*task.state.borrow() {
            TaskState::Completed(result) => return result.clone(),
            TaskState::Running => return Err(format!("task `{}` is already running", task.function.name)),
            TaskState::Pending => {}
        }
        *task.state.borrow_mut() = TaskState::Running;
        let result = self.invoke_function(task.function.clone(), task.args.clone());
        *task.state.borrow_mut() = TaskState::Completed(result.clone());
        result
    }

    fn execute_stmt(&mut self, stmt: &Stmt, env: EnvRef, base_dir: &Path) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Located(span, inner) => match self.execute_stmt(inner, env, base_dir) {
                Err(RuntimeError::Message(message)) => Err(RuntimeError::Located(span.clone(), message)),
                other => other,
            },
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
                let func = FunctionValue {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                    is_async: *is_async,
                    source_dir: base_dir.to_path_buf(),
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
        if !self.loading_modules.insert(canonical.clone()) {
            return Err(format!("cyclic module import detected at {}", canonical.display()));
        }
        let result = (|| {
            let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read module {}: {}", canonical.display(), e))?;
            let source = cached_preprocess(&source, canonical.parent().unwrap_or(base_dir))?;
            let mut parser = Parser::new_with_source(&source, canonical.to_string_lossy());
            let program = parser.parse_program()?;
            let parent = canonical.parent().unwrap_or(base_dir);
            for stmt in &program.stmts {
                self.execute_stmt(stmt, self.globals.clone(), parent).map_err(runtime_error_to_string)?;
            }
            Ok(())
        })();
        self.loading_modules.remove(&canonical);
        if result.is_ok() {
            self.loaded_modules.insert(canonical);
        }
        result
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
            Expr::AssignProperty(target, name, value_expr) => {
                let target_value = self.evaluate(target, env.clone(), base_dir)?;
                let value = self.evaluate(value_expr, env, base_dir)?;
                match target_value {
                    Value::Record(record) => {
                        record.borrow_mut().insert(name.clone(), value.clone());
                        Ok(value)
                    }
                    _ => Err("property assignment requires a record".to_string()),
                }
            }
            Expr::Grouping(inner) => self.evaluate(inner, env, base_dir),
            Expr::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.evaluate(item, env.clone(), base_dir)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::Record(fields) => {
                let mut values = BTreeMap::new();
                for (name, expression) in fields {
                    if values.contains_key(name) {
                        return Err(format!("duplicate record field `{}`", name));
                    }
                    values.insert(name.clone(), self.evaluate(expression, env.clone(), base_dir)?);
                }
                Ok(Value::Record(Rc::new(RefCell::new(values))))
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
            Expr::Property(target, name) => {
                let target_value = self.evaluate(target, env, base_dir)?;
                match target_value {
                    Value::Record(record) => record
                        .borrow()
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("record has no property `{}`", name)),
                    _ => Err("property access requires a record".to_string()),
                }
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
        match target {
            Value::Array(items) => {
                let idx = as_index(&index)?;
                items.borrow().get(idx).cloned().ok_or_else(|| format!("array index {} is out of bounds", idx))
            }
            Value::String(text) => {
                let idx = as_index(&index)?;
                text.chars().nth(idx).map(|c| Value::String(c.to_string())).ok_or_else(|| format!("string index {} is out of bounds", idx))
            }
            Value::Record(record) => {
                let key = value_string_arg(&index, "record index").map_err(runtime_error_to_string)?;
                record.borrow().get(&key).cloned().ok_or_else(|| format!("record has no key `{}`", key))
            }
            _ => Err("index access can only be used on arrays, strings, or records".to_string()),
        }
    }

    fn assign_index(&self, target: Value, index: Value, value: Value) -> Result<(), String> {
        match target {
            Value::Array(items) => {
                let idx = as_index(&index)?;
                let mut items = items.borrow_mut();
                if idx >= items.len() { return Err(format!("array index {} is out of bounds", idx)); }
                items[idx] = value;
                Ok(())
            }
            Value::Record(record) => {
                let key = value_string_arg(&index, "record index").map_err(runtime_error_to_string)?;
                record.borrow_mut().insert(key, value);
                Ok(())
            }
            _ => Err("indexed assignment can only be used on arrays or records".to_string()),
        }
    }

    fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Null => false,
            Value::Bool(v) => *v,
            Value::Number(v) => *v != 0.0,
            Value::String(v) => !v.is_empty(),
            Value::Array(v) => !v.borrow().is_empty(),
            Value::Record(v) => !v.borrow().is_empty(),
            Value::Function(_)
            | Value::Task(_)
            | Value::Listener(_)
            | Value::Connection(_)
            | Value::NativeFunction(_) => true,
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

fn existing_module_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if path.is_dir() {
        let manifest = path.join("oxid.toml");
        if manifest.is_file() {
            if let Ok(project) = load_manifest(&manifest) {
                if let Some(entry) = project.entry {
                    let entry = path.join(entry);
                    if entry.is_file() {
                        return Some(entry);
                    }
                }
            }
        }
        for candidate in ["src/lib.ox", "src/main.ox", "lib.ox", "main.ox", "package.ox"] {
            let candidate = path.join(candidate);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_path(base_dir: &Path, text: &str) -> PathBuf {
    let candidate = Path::new(text);
    if candidate.is_absolute() { return candidate.to_path_buf(); }
    let project_root = project_root_for(base_dir);
    if let Ok(manifest) = load_manifest(&project_root.join("oxid.toml")) {
        if let Some(first) = candidate.components().next().and_then(|part| match part {
            std::path::Component::Normal(name) => name.to_str(),
            _ => None,
        }) {
            if let Some(spec) = manifest.dependencies.get(first) {
                let dependency_root = if spec.starts_with("git+") || spec.starts_with("https://") {
                    project_root.join(".oxid").join("deps").join(first)
                } else {
                    project_root.join(spec)
                };
                let remainder = candidate.strip_prefix(first).unwrap_or(Path::new(""));
                if let Some(module) = existing_module_path(&dependency_root.join(remainder)) {
                    return module;
                }
            }
        }
    }
    let mut roots = vec![base_dir.to_path_buf(), base_dir.join("src"), base_dir.join("stdlib"), base_dir.join("modules"), base_dir.join("deps"), base_dir.join("vendor")];
    roots.push(project_root.join(".oxid").join("deps"));
    roots.push(project_root.join("deps"));
    roots.push(project_root.join("vendor"));
    if let Ok(extra) = env::var("OXID_PATH") { for p in env::split_paths(&extra) { roots.push(p); } }
    for root in roots {
        let joined = root.join(candidate);
        if let Some(module) = existing_module_path(&joined) { return module; }
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
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn cache_root(base_dir: &Path) -> PathBuf {
    if let Ok(path) = env::var("OXID_CACHE_DIR") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
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
            body_lines.push(String::new());
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
    if fs::create_dir_all(&cache_dir).is_err() {
        return preprocess_source(source);
    }
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
    run_source_named(source, base_dir, "<memory>", interp)
}

fn run_source_named(source: &str, base_dir: &Path, source_name: &str, interp: &mut Interpreter) -> Result<(), String> {
    let processed = cached_preprocess(source, base_dir)?;
    let mut parser = Parser::new_with_source(&processed, source_name);
    let program = parser.parse_program()?;
    interp.execute_program(&program, base_dir)
}

fn run_file(path: &Path, interp: &mut Interpreter) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open file: {} ({})", path.display(), e))?;
    let base_dir = canonical.parent().unwrap_or(Path::new("."));
    let bytes = fs::read(&canonical).map_err(|e| format!("cannot read file: {} ({})", canonical.display(), e))?;
    if artifact::is_artifact(&bytes) {
        let decoded = artifact::decode(&bytes)?;
        interp.execute_program(&decoded.program, base_dir)
    } else {
        let source = String::from_utf8(bytes).map_err(|_| format!("source is not valid UTF-8: {}", canonical.display()))?;
        run_source_named(&source, base_dir, &canonical.to_string_lossy(), interp)
    }
}

fn split_script_command(script: &str) -> Result<Vec<String>, String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut has_content = false;
    let mut characters = script.chars().peekable();

    while let Some(character) = characters.next() {
        if character.is_control() && !character.is_whitespace() {
            return Err("script command contains a control character".to_string());
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
                has_content = true;
            } else if character == '\\' && active_quote == '"' {
                match characters.peek().copied() {
                    Some('"' | '\\') => {
                        current.push(characters.next().expect("peeked script character"));
                        has_content = true;
                    }
                    _ => current.push(character),
                }
            } else {
                current.push(character);
                has_content = true;
            }
            continue;
        }

        match character {
            '\'' | '"' => {
                quote = Some(character);
                has_content = true;
            }
            character if character.is_whitespace() => {
                if has_content {
                    arguments.push(std::mem::take(&mut current));
                    has_content = false;
                }
            }
            '\\' => match characters.peek().copied() {
                Some(next) if next.is_whitespace() || matches!(next, '\'' | '"' | '\\') => {
                    current.push(characters.next().expect("peeked script character"));
                    has_content = true;
                }
                _ => {
                    current.push(character);
                    has_content = true;
                }
            },
            _ => {
                current.push(character);
                has_content = true;
            }
        }
    }

    if let Some(active_quote) = quote {
        return Err(format!("unterminated {} quote in script command", active_quote));
    }
    if has_content {
        arguments.push(current);
    }
    if arguments.first().is_none_or(String::is_empty) {
        return Err("script command is empty".to_string());
    }
    Ok(arguments)
}

fn run_manifest_script(root: &Path, script_name: &str, extra_args: &[String]) -> Result<(), String> {
    let manifest = load_manifest(&root.join("oxid.toml"))?;
    let script = manifest.scripts.get(script_name).cloned().ok_or_else(|| format!("script `{}` was not found in oxid.toml", script_name))?;
    let arguments = split_script_command(&script)?;
    let status = Command::new(&arguments[0])
        .args(&arguments[1..])
        .args(extra_args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to launch script `{}`: {}", script_name, e))?;
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

fn parse_manifest_value(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches(',').trim();
    if !trimmed.starts_with('"') {
        return Ok(trimmed.split('#').next().unwrap_or("").trim().to_string());
    }
    let mut output = String::new();
    let mut index = 1usize;
    while index < trimmed.len() {
        let character = trimmed[index..].chars().next().ok_or_else(|| "unterminated manifest string".to_string())?;
        index += character.len_utf8();
        match character {
            '"' => {
                let suffix = trimmed[index..].trim();
                if suffix.is_empty() || suffix.starts_with('#') {
                    return Ok(output);
                }
                return Err(format!("unexpected text after manifest string: {}", suffix));
            }
            '\\' => {
                let escaped = trimmed[index..].chars().next().ok_or_else(|| "unterminated manifest string escape".to_string())?;
                index += escaped.len_utf8();
                match escaped {
                    '"' => output.push('"'),
                    '\\' => output.push('\\'),
                    'n' => output.push('\n'),
                    'r' => output.push('\r'),
                    't' => output.push('\t'),
                    'u' => {
                        let end = index.checked_add(4).ok_or_else(|| "manifest Unicode escape overflow".to_string())?;
                        let digits = trimmed.get(index..end).ok_or_else(|| "incomplete manifest Unicode escape".to_string())?;
                        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                            return Err("manifest Unicode escapes require four hexadecimal digits".to_string());
                        }
                        let value = u32::from_str_radix(digits, 16).map_err(|_| "invalid manifest Unicode escape".to_string())?;
                        output.push(char::from_u32(value).ok_or_else(|| "invalid manifest Unicode scalar".to_string())?);
                        index = end;
                    }
                    other => return Err(format!("unsupported manifest string escape `\\{}`", other)),
                }
            }
            character if character.is_control() => return Err("manifest string contains an unescaped control character".to_string()),
            _ => output.push(character),
        }
    }
    Err("unterminated manifest string".to_string())
}

fn quote_manifest_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push_str(&format!("\\u{:04x}", u32::from(character))),
            _ => output.push(character),
        }
    }
    output.push('"');
    output
}

fn manifest_delimiter(text: &str, delimiter: char) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        if let Some(active) = quote {
            if escaped { escaped = false; }
            else if active == '"' && character == '\\' { escaped = true; }
            else if character == active { quote = None; }
        } else if matches!(character, '"' | '\'') { quote = Some(character); }
        else if character == '#' { return None; }
        else if character == delimiter { return Some(index); }
    }
    None
}

fn parse_manifest_key(raw: &str) -> Result<String, String> {
    if raw.ends_with(',') { return Err("unexpected comma after manifest key".to_string()); }
    if raw.starts_with('"') { return parse_manifest_value(raw); }
    if let Some(literal) = raw.strip_prefix('\'').and_then(|value| value.strip_suffix('\'')) {
        if !literal.contains('\'') && !literal.chars().any(char::is_control) { return Ok(literal.to_string()); }
    }
    if raw.starts_with('\'') { return Err("invalid literal manifest key".to_string()); }
    Ok(raw.to_string())
}

fn is_test_fixture_section(line: &str) -> Result<bool, String> {
    let array = line.starts_with("[[");
    let opening = if array { 2 } else { 1 };
    let closing = manifest_delimiter(&line[opening..], ']').ok_or("missing closing bracket")? + opening;
    let raw = line[opening..closing].trim();
    let suffix = &line[closing + 1..];
    let suffix = if array { suffix.strip_prefix(']').ok_or("missing array-table closing bracket")? } else { suffix };
    if raw.is_empty() || (!suffix.trim().is_empty() && !suffix.trim().starts_with('#')) {
        return Err("empty section or unexpected text after section".to_string());
    }
    let mut names = Vec::new();
    let mut remaining = raw;
    loop {
        let dot = manifest_delimiter(remaining, '.');
        let component = remaining[..dot.unwrap_or(remaining.len())].trim();
        if component.is_empty() || (!component.starts_with(['"', '\'']) && !component.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))) {
            return Err("invalid bare section name".to_string());
        }
        names.push(parse_manifest_key(component)?);
        let Some(dot) = dot else { break; };
        remaining = &remaining[dot + 1..];
    }
    if names[0] != "test-fixtures" { return Ok(false); }
    if array || names.len() != 1 { return Err("test-fixtures must be one ordinary table".to_string()); }
    Ok(true)
}

// Track only lexical continuation in unrelated values. Their semantics remain
// the owning consumer's concern, and apparent headers inside data are inert.
#[derive(Default)]
struct ManifestValueContinuation {
    quote: Option<(u8, bool)>,
    depth: usize,
}

impl ManifestValueContinuation {
    fn active(&self) -> bool { self.quote.is_some() || self.depth != 0 }

    fn scan(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            if let Some((quote, triple)) = self.quote {
                if quote == b'"' && byte == b'\\' { index += 2; continue; }
                if byte == quote {
                    let count = bytes[index..].iter().take_while(|&&value| value == quote).count();
                    if !triple || count >= 3 {
                        self.quote = None;
                        index += if triple { if count <= 5 { count } else { 3 } } else { 1 };
                        continue;
                    }
                }
            } else {
                match byte {
                    b'#' => break,
                    b'"' | b'\'' => {
                        let triple = bytes.get(index..index + 3) == Some(&[byte, byte, byte]);
                        self.quote = Some((byte, triple));
                        index += if triple { 3 } else { 1 };
                        continue;
                    }
                    b'[' | b'{' => self.depth += 1,
                    b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
            index += 1;
        }
    }
}

fn load_test_fixtures(path: &Path) -> Result<HashSet<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read manifest: {} ({})", path.display(), e))?;
    let mut fixtures = HashSet::new();
    let mut fixture_section = false;
    let mut continuation = ManifestValueContinuation::default();
    for raw_line in text.lines() {
        if continuation.active() { continuation.scan(raw_line); continue; }
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') {
            fixture_section = is_test_fixture_section(line).map_err(|error| format!("invalid manifest section in {}: {}", path.display(), error))?;
            continue;
        }
        let equals = manifest_delimiter(line, '=');
        if !fixture_section {
            if let Some(equals) = equals { continuation.scan(&line[equals + 1..]); }
            continue;
        }
        let equals = equals.ok_or("test fixture entry requires `\"relative/file.ox\" = true`")?;
        let key_raw = line[..equals].trim();
        let value_raw = &line[equals + 1..];
        let key = parse_manifest_key(key_raw).map_err(|error| format!("invalid manifest key in {}: {}", path.display(), error))?;
        if !key_raw.starts_with('"') || value_raw.split('#').next().unwrap_or("").trim() != "true" {
            return Err("test fixture entry requires `\"relative/file.ox\" = true`".to_string());
        }
        if !(key.starts_with("tests/") || key.starts_with("examples/"))
            || !key.ends_with(".ox") || key.contains(['\\', ':', '*', '?', '[', ']'])
            || key.chars().any(char::is_control)
            || key.split('/').any(|part| matches!(part, "" | "." | "..")) {
            return Err(format!("test fixture must be an exact relative .ox file under tests/ or examples/: {}", key));
        }
        if !fixtures.insert(key.clone()) { return Err(format!("duplicate test fixture: {}", key)); }
    }
    if continuation.active() { return Err("unterminated manifest value while reading test fixtures".to_string()); }
    Ok(fixtures)
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
        let Some((key_raw, value_raw)) = line.split_once('=') else { continue; };
        let key_raw = key_raw.trim();
        let key = if key_raw.starts_with('"') {
            parse_manifest_value(key_raw).map_err(|error| format!("invalid manifest key in {}: {}", path.display(), error))?
        } else {
            key_raw.to_string()
        };
        let value = parse_manifest_value(value_raw).map_err(|error| format!("invalid manifest value in {}: {}", path.display(), error))?;
        match (section.as_str(), key.as_str()) {
            ("project", "name") | ("", "name") => manifest.name = Some(value),
            ("project", "version") | ("", "version") => manifest.version = Some(value),
            ("project", "entry") | ("build", "entry") | ("", "entry") => manifest.entry = Some(value),
            ("features", _) => {
                let enabled = matches!(value.as_str(), "true" | "yes" | "on" | "1");
                manifest.features.insert(key, enabled);
            }
            ("scripts", _) => { manifest.scripts.insert(key, value); }
            ("dependencies", _) => { manifest.dependencies.insert(key, value); }
            _ => {}
        }
    }
    Ok(manifest)
}

fn project_root_for(path: &Path) -> PathBuf {
    let start = if path.is_dir() { path } else { path.parent().unwrap_or(Path::new(".")) };
    for ancestor in start.ancestors() {
        if ancestor.join("oxid.toml").is_file() {
            return ancestor.to_path_buf();
        }
    }
    start.to_path_buf()
}

fn portable_source_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn compile_module_graph(
    path: &Path,
    root: &Path,
    loading: &mut HashSet<PathBuf>,
    loaded: &mut HashSet<PathBuf>,
    statements: &mut Vec<Stmt>,
) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(|e| format!("cannot open source {}: {}", path.display(), e))?;
    if loaded.contains(&canonical) {
        return Ok(());
    }
    if !loading.insert(canonical.clone()) {
        return Err(format!(
            "cyclic module import detected while compiling {}",
            portable_source_name(root, &canonical)
        ));
    }
    let result = (|| {
        let source = fs::read_to_string(&canonical).map_err(|e| format!("cannot read source {}: {}", canonical.display(), e))?;
        let processed = preprocess_source(&source)?;
        let source_name = portable_source_name(root, &canonical);
        let mut parser = Parser::new_with_source(&processed, source_name);
        let program = parser.parse_program()?;
        let base_dir = canonical.parent().unwrap_or(root);
        let mut local = Vec::new();
        for statement in program.stmts {
            let import = match &statement {
                Stmt::Located(_, inner) => match inner.as_ref() {
                    Stmt::Use(module) => Some(module.clone()),
                    _ => None,
                },
                Stmt::Use(module) => Some(module.clone()),
                _ => None,
            };
            if let Some(module) = import {
                let resolved = resolve_path(base_dir, &module);
                compile_module_graph(&resolved, root, loading, loaded, statements)?;
            } else {
                local.push(statement);
            }
        }
        statements.extend(local);
        Ok(())
    })();
    loading.remove(&canonical);
    if result.is_ok() {
        loaded.insert(canonical);
    }
    result
}

fn compile_program(input: &Path) -> Result<(Program, u32), String> {
    let canonical = fs::canonicalize(input).map_err(|e| format!("cannot open source {}: {}", input.display(), e))?;
    let root = project_root_for(&canonical);
    let mut loading = HashSet::new();
    let mut loaded = HashSet::new();
    let mut statements = Vec::new();
    compile_module_graph(&canonical, &root, &mut loading, &mut loaded, &mut statements)?;
    Ok((Program { stmts: statements }, loaded.len() as u32))
}

fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    let (program, module_count) = compile_program(input)?;
    if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| format!("cannot create output directory: {}", e))?; }
    artifact::write(output, &program, module_count)?;
    println!("compiled: {} -> {} ({} modules, bytecode v1)", input.display(), output.display(), module_count);
    Ok(())
}

fn sorted_map_text<T: ToString>(values: &HashMap<String, T>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    let mut entries = values.iter().map(|(key, value)| format!("{}={}", key, value.to_string())).collect::<Vec<_>>();
    entries.sort();
    entries.join(", ")
}

fn build_project(root: &Path, options: packages::ResolveOptions) -> Result<(), String> {
    let manifest_path = root.join("oxid.toml");
    if !manifest_path.exists() { return Err(format!("manifest not found: {}", manifest_path.display())); }
    let manifest = load_manifest(&manifest_path)?;
    let locked_packages = packages::resolve_dependencies(root, &manifest.dependencies, options)?;
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
        sorted_map_text(&manifest.features),
        sorted_map_text(&manifest.scripts),
        if locked_packages.is_empty() {
            "none".to_string()
        } else {
            locked_packages.iter().map(|package| format!("{}@{}", package.name, package.checksum)).collect::<Vec<_>>().join(", ")
        },
    );
    let oxid_dir = root.join(".oxid");
    fs::create_dir_all(&oxid_dir).map_err(|e| format!("cannot create build directory: {}", e))?;
    fs::write(oxid_dir.join("build-report.txt"), report).map_err(|e| format!("cannot write build report: {}", e))?;
    let artifact_name = manifest.name.as_deref().unwrap_or("app");
    compile_file(&entry_path, &oxid_dir.join("bin").join(format!("{}.oxb", artifact_name)))?;
    println!("build ok: {}", entry_path.display());
    Ok(())
}

fn resolve_project_dependencies(root: &Path, options: packages::ResolveOptions) -> Result<(), String> {
    let manifest = load_manifest(&root.join("oxid.toml"))?;
    let resolved = packages::resolve_dependencies(root, &manifest.dependencies, options)?;
    if resolved.is_empty() {
        println!("dependencies resolved: none");
    } else {
        for package in &resolved {
            println!("{} {} {}", package.name, package.revision.as_deref().unwrap_or("path"), package.checksum);
        }
        println!("dependencies resolved: {}", resolved.len());
    }
    Ok(())
}

fn list_project_dependencies(root: &Path) -> Result<(), String> {
    let dependencies = packages::list_dependencies(&root.join("oxid.toml"))?;
    if dependencies.is_empty() {
        println!("dependencies: none");
    } else {
        for (name, source) in dependencies {
            println!("{} = {}", name, source);
        }
    }
    Ok(())
}

fn remove_project_dependency(root: &Path, name: &str) -> Result<(), String> {
    if !packages::remove_dependency_from_manifest(&root.join("oxid.toml"), name)? {
        return Err(format!("dependency `{}` is not declared", name));
    }
    resolve_project_dependencies(root, packages::ResolveOptions::default())?;
    println!("removed dependency: {}", name);
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
    let manifest_path = root.join("oxid.toml");
    let mut fixtures = HashSet::new();
    if manifest_path.is_file() {
        for relative in load_test_fixtures(&manifest_path)? {
            let mut path = root.to_path_buf();
            for part in relative.split('/') {
                path.push(part);
                let metadata = fs::symlink_metadata(&path).map_err(|e| format!("cannot read test fixture: {} ({})", path.display(), e))?;
                if metadata.file_type().is_symlink() {
                    return Err(format!("test fixture cannot contain a symlink: {}", path.display()));
                }
            }
            if !path.is_file() { return Err(format!("test fixture must be a file: {}", path.display())); }
            fixtures.insert(path);
        }
    }
    let mut files = Vec::new();
    let tests_dir = root.join("tests");
    if tests_dir.exists() { files.extend(collect_oxid_files(&tests_dir).into_iter().filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ox"))); }
    let examples_dir = root.join("examples");
    if examples_dir.exists() { files.extend(collect_oxid_files(&examples_dir).into_iter().filter(|p| p.extension().and_then(|s| s.to_str()) == Some("ox"))); }
    files.retain(|path| !fixtures.contains(path));
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
    let manifest_path = root.join("oxid.toml");
    println!("manifest: {}", if manifest_path.is_file() { "ok" } else { "missing" });
    if !manifest_path.is_file() {
        return Err(format!("project health check failed; missing: {}", manifest_path.display()));
    }
    let manifest = load_manifest(&manifest_path)?;
    let entry = manifest
        .entry
        .as_deref()
        .map(|path| root.join(path))
        .or_else(|| [root.join("src/main.ox"), root.join("main.ox")].into_iter().find(|path| path.is_file()))
        .ok_or_else(|| "project health check failed; no project entry is configured".to_string())?;
    let repository_mode = root.join("Cargo.toml").is_file() && root.join("native").is_dir();
    let mut checks = vec![
        ("entry", entry, true),
        ("readme", root.join("README.md"), false),
        ("examples", root.join("examples"), false),
        ("stdlib", root.join("stdlib"), false),
        ("tests", root.join("tests"), false),
        ("lockfile", root.join("oxid.lock"), !manifest.dependencies.is_empty()),
    ];
    if repository_mode {
        checks.extend([
            ("readme-zh", root.join("README_ZH.md"), true),
            ("readme-jp", root.join("README_JP.md"), true),
            ("native", root.join("native"), true),
            ("docs", root.join("docs"), true),
            ("compiler", root.join("compiler/main.ox"), true),
            ("ci", root.join(".github/workflows/ci.yml"), true),
            ("release", root.join(".github/workflows/release.yml"), true),
            ("unix-installer", root.join("install.sh"), true),
            ("windows-installer", root.join("install.ps1"), true),
        ]);
    }
    let mut missing = Vec::new();
    for (name, path, required) in checks {
        let exists = path.exists();
        println!("{}: {}", name, if exists { "ok" } else if required { "missing" } else { "optional" });
        if required && !exists { missing.push(name); }
    }
    let lockfile = root.join("oxid.lock");
    if lockfile.is_file() {
        packages::read_lockfile(&lockfile)?;
        println!("lockfile-format: ok");
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
- json_parse / json_stringify
- record / keys / has_key / get / set / remove
- process / process_output / python / java / go
- net_listen / net_local_addr / net_accept / net_try_accept / net_read / net_write / net_close
- http_read_request / http_write_response
- web_response / web_serve_once
- c_len / c_hash / cpp_len / cpp_hash

## Commands

- oxid run
- oxid script
- oxid repl
- oxid check
- oxid compile
- oxid ast
- oxid inspect
- oxid watch
- oxid build / install
- oxid lock / fetch / update
- oxid list / remove
- oxid clean
- oxid fmt
- oxid test
- oxid doctor
- oxid doc
- oxid new
- oxid init
- oxid add
- oxid bench
- oxid bootstrap / self-compile / self-host
- oxid frontend / lint
- oxid bridge
- oxid web new
- oxid discord new

## Language focus

- source interpretation
- lazy, memoized tasks; join_all executes sequentially
- concise fun / var / say / give / when / for syntax
- deterministic OXBC 1.0 serialized-AST artifacts executed by the runtime
- bootstrap aliases verify artifact serialization round-trip, not compiler self-rebuild
- the production parser and binary artifact writer are Rust implementations
- cross-module source spans and pipeline expressions
- macro pre-expansion
- local and locked Git module loading
- records, canonical JSON, and nonblocking network adapters
- Python, Java, Go, C, and C++ interoperability
- Web routing and Discord interaction modules
"#;
    fs::write(docs.join("API.md"), api).map_err(|e| format!("cannot write docs: {}", e))?;
    Ok(())
}

fn scaffold_project(name: &str) -> Result<(), String> {
    let root = Path::new(name);
    if root.exists() { return Err(format!("already exists: {}", root.display())); }
    let project_name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "project name must end with a valid UTF-8 directory name".to_string())?;
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
name = {}
version = {}
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
"#, quote_manifest_string(project_name), quote_manifest_string(OXID_VERSION));
    fs::write(root.join("oxid.toml"), manifest).map_err(|e| format!("failed to create oxid.toml: {}", e))?;
    packages::write_lockfile(root, &[]).map_err(|e| format!("failed to create oxid.lock: {}", e))?;
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
    if name.is_empty()
        || !name.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("dependency name may contain only ASCII letters, digits, '-', and '_'".to_string());
    }
    if target.trim().is_empty() || target.chars().any(|character| character.is_control() || character == '"') {
        return Err("dependency target is empty or contains unsafe TOML characters".to_string());
    }
    let path = root.join("oxid.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("cannot read manifest: {} ({})", path.display(), e))?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    let dep_section = lines.iter().position(|line| line.trim() == "[dependencies]");
    let entry = format!("{} = {}", quote_manifest_string(name), quote_manifest_string(target));
    if packages::list_dependencies(&path)?.iter().any(|(existing, _)| existing == name) {
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
        Value::Record(fields) => Ok(Value::Number(fields.borrow().len() as f64)),
        _ => Err(RuntimeError::Message("len can only be used with strings, arrays, or records".to_string())),
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
            Ok(Value::Task(Rc::new(TaskValue {
                function: func.clone(),
                args: extra,
                state: RefCell::new(TaskState::Pending),
            })))
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
            let task = Rc::new(TaskValue {
                function: func.clone(),
                args: Vec::new(),
                state: RefCell::new(TaskState::Pending),
            });
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
            Value::Function(func) => interp.execute_task(Rc::new(TaskValue {
                function: func,
                args: Vec::new(),
                state: RefCell::new(TaskState::Pending),
            })).map_err(RuntimeError::Message)?,
            _ => return Err(RuntimeError::Message("join_all accepts only tasks or functions".to_string())),
        };
        results.push(value);
    }
    Ok(Value::Array(Rc::new(RefCell::new(results))))
}

fn native_task_status(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 { return Err(RuntimeError::Message("task_status requires 1 argument".to_string())); }
    match &args[0] {
        Value::Task(task) => {
            let status = match &*task.state.borrow() {
                TaskState::Pending => "pending",
                TaskState::Running => "running",
                TaskState::Completed(Ok(_)) => "completed",
                TaskState::Completed(Err(_)) => "failed",
            };
            Ok(Value::String(status.to_string()))
        }
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
        Value::Record(_) => "record",
        Value::Function(_) => "function",
        Value::Task(_) => "task",
        Value::Listener(_) => "listener",
        Value::Connection(_) => "connection",
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
    if !(100..=599).contains(&status) {
        return Err(RuntimeError::Message("web status must be between 100 and 599".to_string()));
    }
    let content_type = value_string_arg(&args[1], "content type")?;
    if content_type.is_empty() || content_type.len() > 255 || content_type.chars().any(char::is_control) {
        return Err(RuntimeError::Message("content type is empty, too long, or contains control characters".to_string()));
    }
    let body = value_string_arg(&args[2], "response body")?;
    let reason = match status { 200 => "OK", 201 => "Created", 204 => "No Content", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden", 404 => "Not Found", 500 => "Internal Server Error", _ => "Response" };
    Ok(Value::String(format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, reason, content_type, body.len(), body)))
}

fn native_web_serve_once(args: Vec<Value>) -> Result<Value, RuntimeError> {
    network::web_serve_once(args)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => *v != 0.0,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.borrow().is_empty(),
        Value::Record(v) => !v.borrow().is_empty(),
        Value::Function(_)
        | Value::Task(_)
        | Value::Listener(_)
        | Value::Connection(_)
        | Value::NativeFunction(_) => true,
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
        (Value::Record(a), Value::Record(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.len() == b.len()
                && a.iter().all(|(key, value)| b.get(key).is_some_and(|other| values_equal(value, other)))
        }
        (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
        (Value::Task(a), Value::Task(b)) => Rc::ptr_eq(a, b),
        (Value::Listener(a), Value::Listener(b)) => Rc::ptr_eq(a, b),
        (Value::Connection(a), Value::Connection(b)) => Rc::ptr_eq(a, b),
        (Value::NativeFunction(a), Value::NativeFunction(b)) => std::ptr::fn_addr_eq(*a, *b),
        _ => false,
    }
}

fn strip_top_level_imports(program: Program) -> Program {
    Program {
        stmts: program
            .stmts
            .into_iter()
            .filter(|statement| match statement {
                Stmt::Use(_) => false,
                Stmt::Located(_, inner) => !matches!(inner.as_ref(), Stmt::Use(_)),
                _ => true,
            })
            .collect(),
    }
}

fn embedded_compiler_program() -> Result<(Program, u32), String> {
    const EMITTER_SOURCE: &str = include_str!("../stdlib/frontend/bytecode.ox");
    const COMPILER_SOURCE: &str = include_str!("../compiler/main.ox");
    let mut emitter = Parser::new_with_source(EMITTER_SOURCE, "stdlib/frontend/bytecode.ox");
    let mut compiler = Parser::new_with_source(COMPILER_SOURCE, "compiler/main.ox");
    let mut statements = emitter.parse_program()?.stmts;
    statements.extend(strip_top_level_imports(compiler.parse_program()?).stmts);
    Ok((Program { stmts: statements }, 2))
}

fn bootstrap_compiler_program(root: &Path) -> Result<(Program, u32), String> {
    let entry = root.join("compiler/main.ox");
    if entry.is_file() {
        compile_program(&entry)
    } else {
        embedded_compiler_program()
    }
}

fn atomic_write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {}", parent.display(), error))?;
    }
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("artifact");
    let temporary = path.with_file_name(format!(".{}.{}.tmp", file_name, std::process::id()));
    fs::write(&temporary, bytes).map_err(|error| format!("cannot write {}: {}", temporary.display(), error))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("cannot replace {}: {}", path.display(), error))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("cannot publish {}: {}", path.display(), error))
}

fn verify_provider_manifest(text: &str) -> Result<(), String> {
    for required in [
        "schema_version = 1",
        "[providers]",
        "emitter = \"oxid\"",
        "lexer = \"stage0\"",
        "parser = \"stage0\"",
        "diagnostics = \"stage0\"",
        "modules = \"stage0\"",
        "[parity]",
        "gate = \"byte-for-byte\"",
        "required_before_each_switch = true",
        "[components.emitter]",
        "entry = \"stdlib/frontend/bytecode.ox\"",
    ] {
        if !text.contains(required) {
            return Err(format!("compiler provider manifest is missing `{}`", required));
        }
    }
    Ok(())
}

fn bootstrap_project(root: &Path, write_artifacts: bool) -> Result<(), String> {
    const EMBEDDED_PROVIDERS: &str = include_str!("../compiler/providers.toml");
    let provider_path = root.join("compiler/providers.toml");
    let provider_text = fs::read_to_string(&provider_path).unwrap_or_else(|_| EMBEDDED_PROVIDERS.to_string());
    verify_provider_manifest(&provider_text)?;

    let (program, module_count) = bootstrap_compiler_program(root)?;
    let stage0 = artifact::encode(&program, module_count)?;
    let decoded0 = artifact::decode(&stage0)?;
    let stage1 = artifact::encode(&decoded0.program, decoded0.module_count)?;
    let decoded1 = artifact::decode(&stage1)?;
    let stage2 = artifact::encode(&decoded1.program, decoded1.module_count)?;
    if stage0 != stage1 {
        return Err("Stage-0/Stage-1 compiler artifacts differ".to_string());
    }
    if stage1 != stage2 {
        return Err("Stage-1/Stage-2 compiler fixed point failed".to_string());
    }

    let mut interpreter = Interpreter::new();
    interpreter.execute_program(&decoded1.program, root)?;
    let checksum = artifact::checksum_hex(&stage1);
    if write_artifacts {
        let output = root.join(".oxid/bootstrap");
        atomic_write_file(&output.join("stage0.oxb"), &stage0)?;
        atomic_write_file(&output.join("stage1.oxb"), &stage1)?;
        atomic_write_file(&output.join("stage2.oxb"), &stage2)?;
        atomic_write_file(&output.join("compiler.oxb"), &stage1)?;
        let manifest = format!(
            "{{\n  \"schema_version\": 1,\n  \"artifact_format\": 1,\n  \"ast_format\": 1,\n  \"compiler_version\": \"{}\",\n  \"modules\": {},\n  \"checksum\": \"{}\",\n  \"stage0_stage1_equal\": true,\n  \"stage1_stage2_equal\": true\n}}\n",
            OXID_VERSION, module_count, checksum
        );
        atomic_write_file(&output.join("manifest.json"), manifest.as_bytes())?;
    }
    println!("bootstrap verified: stage0 == stage1 == stage2 ({})", checksum);
    Ok(())
}

fn inspect_artifact(path: &Path) -> Result<(), String> {
    let decoded = artifact::read(path)?;
    println!("artifact: {}", path.display());
    println!("format: OXBC 1.0");
    println!("ast: 1");
    println!("modules: {}", decoded.module_count);
    println!("statements: {}", decoded.program.stmts.len());
    println!("checksum: {:016x}", decoded.checksum);
    Ok(())
}

extern "C" {
    fn oxid_c_strlen(s: *const c_char) -> usize;
    fn oxid_c_hash(s: *const c_char) -> u64;
    fn oxid_cpp_len(s: *const c_char) -> usize;
    fn oxid_cpp_hash(s: *const c_char) -> u64;
}

fn help() {
    println!("Oxid {}", OXID_VERSION);
    println!("Usage:");
    println!("  oxid run <file.ox|file.oxb>");
    println!("  oxid script <name> [args...]");
    println!("  oxid check <file.ox> [--edition typed-preview --message-format text|json]");
    println!("  --edition legacy-0.9 keeps the default legacy route; typed-preview is check-only");
    println!("  oxid compile <file.ox> [-o app.oxb]");
    println!("  oxid ast <file.ox> [-o app.oxa]");
    println!("  oxid inspect <file.oxb>");
    println!("  oxid repl");
    println!("  oxid new <project-name>");
    println!("  oxid init <project-name>");
    println!("  oxid add <name> <path-or-target>");
    println!("  oxid remove <name>");
    println!("  oxid lock [--offline|--locked]");
    println!("  oxid fetch [--offline|--locked]");
    println!("  oxid update");
    println!("  oxid install [--offline|--locked]");
    println!("  oxid list");
    println!("  oxid bridge <python|java|go|c|cpp|all> [output]");
    println!("  oxid web new <project-name>");
    println!("  oxid discord new <project-name>");
    println!("  oxid watch <file.ox>");
    println!("  oxid build [--offline|--locked|--frozen]");
    println!("  oxid clean");
    println!("  oxid fmt [path]");
    println!("  oxid test");
    println!("  oxid doctor");
    println!("  oxid doc");
    println!("  oxid bench [--iterations N] [--json report.json]");
    println!("  oxid bootstrap [--check]");
    println!("  oxid emit [--check]");
    println!("  oxid self-compile [--check]");
    println!("  oxid self-host [--check]");
    println!("  oxid frontend");
    println!("  oxid diagnose");
    println!("  oxid lint");
    println!("  oxid module");
    println!("  oxid syntax");
    println!("  oxid interop");
    println!("  oxid help");
}

fn check_file(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read file: {} ({})", path.display(), error))?;
    if artifact::is_artifact(&bytes) {
        artifact::decode(&bytes)?;
        println!("artifact ok: {}", path.display());
        return Ok(());
    }
    let source = String::from_utf8(bytes).map_err(|_| format!("source is not valid UTF-8: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or(Path::new("."));
    let source = cached_preprocess(&source, base_dir)?;
    let mut parser = Parser::new_with_source(&source, path.to_string_lossy());
    parser.parse_program()?;
    println!("syntax ok: {}", path.display());
    Ok(())
}

fn compile_command(arguments: &[String], ast_only: bool) -> Result<(), String> {
    let input = arguments.first().ok_or_else(|| {
        if ast_only { "`oxid ast` requires a source file" } else { "`oxid compile` requires a source file" }.to_string()
    })?;
    let mut output = Path::new(input).with_extension(if ast_only { "oxa" } else { "oxb" });
    let mut index = 1usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "-o" | "--output" => {
                let value = arguments.get(index + 1).ok_or_else(|| "`-o` requires an output path".to_string())?;
                output = PathBuf::from(value);
                index += 2;
            }
            unknown => return Err(format!("unknown compile option: {}", unknown)),
        }
    }
    compile_file(Path::new(input), &output)?;
    if ast_only {
        println!("serialized AST: {}", output.display());
    }
    Ok(())
}

fn dependency_options(arguments: &[String]) -> Result<packages::ResolveOptions, String> {
    let mut options = packages::ResolveOptions::default();
    for argument in arguments {
        match argument.as_str() {
            "--locked" => options.locked = true,
            "--offline" => options.offline = true,
            "--frozen" => {
                options.locked = true;
                options.offline = true;
            }
            "--update" => options.update = true,
            unknown => return Err(format!("unknown dependency option: {}", unknown)),
        }
    }
    Ok(options)
}

fn benchmark_command(arguments: &[String]) -> Result<(), String> {
    let mut iterations = 20usize;
    let mut output = None;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--iterations" | "-n" => {
                let value = arguments.get(index + 1).ok_or_else(|| "--iterations requires a value".to_string())?;
                iterations = value.parse::<usize>().map_err(|_| "benchmark iterations must be an integer".to_string())?;
                index += 2;
            }
            "--json" => {
                let value = arguments.get(index + 1).ok_or_else(|| "--json requires an output path".to_string())?;
                output = Some(PathBuf::from(value));
                index += 2;
            }
            unknown => return Err(format!("unknown benchmark option: {}", unknown)),
        }
    }
    benchmark::run(Path::new("."), iterations, output.as_deref())
}

fn lint_project(root: &Path) -> Result<(), String> {
    let files = collect_oxid_files(root)
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("ox"))
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Err("no Oxid source files found".to_string());
    }
    for file in &files {
        check_file(file)?;
    }
    println!("lint ok: {} Oxid sources", files.len());
    Ok(())
}

fn print_frontend_status(root: &Path) -> Result<(), String> {
    const EMBEDDED_PROVIDERS: &str = include_str!("../compiler/providers.toml");
    let manifest = fs::read_to_string(root.join("compiler/providers.toml")).unwrap_or_else(|_| EMBEDDED_PROVIDERS.to_string());
    verify_provider_manifest(&manifest)?;
    print!("{}", manifest);
    Ok(())
}

fn main(args: Vec<String>) {
    let mut interp = Interpreter::new();
    let result = match args.get(1).map(|s| s.as_str()) {
        None | Some("help") | Some("--help") | Some("-h") => { help(); Ok(()) }
        Some("--version") | Some("-V") => { println!("Oxid {}", OXID_VERSION); Ok(()) }
        Some("run") => match args.get(2) {
            Some(file) => run_file(Path::new(file), &mut interp),
            None => Err("`oxid run` requires a file path".to_string()),
        },
        Some("script") => match args.get(2) {
            Some(name) => run_manifest_script(Path::new("."), name, &args[3..]),
            None => Err("`oxid script` requires a script name".to_string()),
        },
        Some("check") => args.get(2).map(|file| check_file(Path::new(file))).unwrap_or_else(|| Err("`oxid check` requires a file path".to_string())),
        Some("compile") => compile_command(&args[2..], false),
        Some("ast") => compile_command(&args[2..], true),
        Some("inspect") => args.get(2).map(|file| inspect_artifact(Path::new(file))).unwrap_or_else(|| Err("`oxid inspect` requires an artifact path".to_string())),
        Some("repl") => repl(&mut interp),
        Some("new") => match args.get(2) { Some(name) => scaffold_project(name), None => Err("`oxid new` requires a project name".to_string()) },
        Some("init") => match args.get(2) { Some(name) => scaffold_project(name), None => Err("`oxid init` requires a project name".to_string()) },
        Some("add") => match (args.get(2), args.get(3)) {
            (Some(name), Some(target)) => add_dependency(Path::new("."), name, target)
                .and_then(|_| resolve_project_dependencies(Path::new("."), packages::ResolveOptions::default())),
            _ => Err("`oxid add` requires a dependency name and target".to_string()),
        },
        Some("remove") => args
            .get(2)
            .map(|name| remove_project_dependency(Path::new("."), name))
            .unwrap_or_else(|| Err("`oxid remove` requires a dependency name".to_string())),
        Some("list") => list_project_dependencies(Path::new(".")),
        Some("lock") | Some("fetch") => dependency_options(&args[2..])
            .and_then(|options| resolve_project_dependencies(Path::new("."), options)),
        Some("update") => resolve_project_dependencies(
            Path::new("."),
            packages::ResolveOptions { locked: false, offline: false, update: true },
        ),
        Some("install") => dependency_options(&args[2..])
            .and_then(|options| build_project(Path::new("."), options)),
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
        Some("build") => dependency_options(&args[2..])
            .and_then(|options| build_project(Path::new("."), options)),
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
        Some("bench") => benchmark_command(&args[2..]),
        Some("bootstrap") | Some("self-compile") | Some("emit") => {
            let write_artifacts = !args[2..].iter().any(|argument| argument == "--check");
            if args[2..].iter().any(|argument| argument != "--check") {
                Err("bootstrap accepts only --check".to_string())
            } else {
                bootstrap_project(Path::new("."), write_artifacts)
            }
        }
        Some("self-host") => {
            let write_artifacts = !args[2..].iter().any(|argument| argument == "--check");
            if args[2..].iter().any(|argument| argument != "--check") {
                Err("self-host accepts only --check".to_string())
            } else {
                bootstrap_project(Path::new("."), write_artifacts)
            }
        }
        Some("frontend") => print_frontend_status(Path::new(".")),
        Some("diagnose") => doctor_project(Path::new(".")),
        Some("lint") => lint_project(Path::new(".")),
        Some("module") => {
            println!("module roots: source, src, stdlib, modules, .oxid/deps, deps, vendor, OXID_PATH");
            Ok(())
        }
        Some("syntax") => {
            println!("syntax: classic fn/let/print/return and concise fun/var/say/give are interchangeable");
            println!("data: arrays, records, property access, indexing, and JSON");
            Ok(())
        }
        Some("interop") => {
            println!("interop: process, Python, Java, Go, C, and C++ bridges");
            Ok(())
        }
        Some(file) if Path::new(file).is_file() => run_file(Path::new(file), &mut interp),
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
        let bundle = fs::read(&output).expect("bundle output");
        assert!(artifact::is_artifact(&bundle));
        let decoded = artifact::decode(&bundle).expect("decode bundle");
        assert_eq!(decoded.module_count, 2);
        assert_eq!(artifact::encode(&decoded.program, decoded.module_count).expect("re-encode"), bundle);
        let mut interpreter = Interpreter::new();
        run_file(&output, &mut interpreter).expect("run bundle");
        assert!(matches!(interpreter.get_var("result"), Ok(Value::Number(number)) if number == 42.0));
        fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn record_json_property_roundtrip_is_deterministic() {
        let mut interpreter = Interpreter::new();
        run_source(
            r#"
const payload = {z: [1, true, null], profile: {name: "Oxid", count: 2}};
payload.profile.count = 3;
const encoded = json_stringify(payload);
const decoded = json_parse(encoded);
const answer = decoded.profile.name + ":" + str(decoded.profile.count);
"#,
            Path::new("."),
            &mut interpreter,
        )
        .expect("record and JSON program");
        assert!(matches!(interpreter.get_var("answer"), Ok(Value::String(value)) if value == "Oxid:3"));
        assert!(matches!(
            interpreter.get_var("encoded"),
            Ok(Value::String(value)) if value == r#"{"profile":{"count":3,"name":"Oxid"},"z":[1,true,null]}"#
        ));
    }

    #[test]
    fn compiled_import_errors_preserve_module_source_ranges() {
        let root = unique_temp_dir("source-spans");
        fs::create_dir_all(&root).expect("temp project");
        fs::write(root.join("lib.ox"), "const valid = 1;\nconst failure = 1 / 0;\n").expect("module");
        fs::write(root.join("main.ox"), "import \"lib.ox\";\n").expect("entry");
        let output = root.join("app.oxb");
        compile_file(&root.join("main.ox"), &output).expect("compile bundle");
        let error = run_file(&output, &mut Interpreter::new()).expect_err("module runtime error");
        assert!(error.contains("lib.ox:2:1-2:"), "unexpected diagnostic: {error}");
        assert!(error.contains("division by zero"), "unexpected diagnostic: {error}");
        fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn compile_rejects_cyclic_module_graphs() {
        let root = unique_temp_dir("compile-cycle");
        fs::create_dir_all(&root).expect("temp project");
        fs::write(root.join("a.ox"), "import \"b.ox\";\n").expect("module a");
        fs::write(root.join("b.ox"), "import \"a.ox\";\n").expect("module b");
        let error = compile_program(&root.join("a.ox")).expect_err("cycle must fail");
        assert!(error.contains("cyclic module import detected"));
        fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn tasks_transition_once_and_memoize_results() {
        let mut interpreter = Interpreter::new();
        run_source(
            r#"
async fun compute() { give 42; }
const task = spawn(compute);
const before = task_status(task);
const first = join(task);
const after = task_status(task);
const second = join(task);
"#,
            Path::new("."),
            &mut interpreter,
        )
        .expect("task lifecycle");
        assert!(matches!(interpreter.get_var("before"), Ok(Value::String(value)) if value == "pending"));
        assert!(matches!(interpreter.get_var("after"), Ok(Value::String(value)) if value == "completed"));
        assert!(matches!(interpreter.get_var("first"), Ok(Value::Number(value)) if value == 42.0));
        assert!(matches!(interpreter.get_var("second"), Ok(Value::Number(value)) if value == 42.0));
    }

    #[test]
    fn script_commands_are_tokenized_without_a_shell() {
        let arguments = split_script_command(r#"tool "two words" 'three words' C:\temp escaped\ value && next"#)
            .expect("script command");
        assert_eq!(
            arguments,
            ["tool", "two words", "three words", r#"C:\temp"#, "escaped value", "&&", "next"]
        );
        assert!(split_script_command("tool \"unterminated").is_err());
    }

    #[test]
    fn generated_project_has_a_valid_lockfile_and_health_check() {
        let root = unique_temp_dir("new-project");
        scaffold_project(root.to_string_lossy().as_ref()).expect("scaffold project");
        assert_eq!(packages::read_lockfile(&root).expect("lockfile"), Vec::new());
        doctor_project(&root).expect("generated project health");
        fs::remove_dir_all(&root).expect("remove generated project");
    }

    #[test]
    fn dependency_edits_preserve_toml_keys_and_windows_paths() {
        let root = unique_temp_dir("manifest-quoting");
        fs::create_dir_all(&root).expect("temp project");
        fs::write(
            root.join("oxid.toml"),
            "[project]\nname = \"manifest-test\"\nentry = \"main.ox\"\n\n[dependencies]\n",
        )
        .expect("manifest");
        fs::write(root.join("main.ox"), "const ready = true;\n").expect("entry");
        add_dependency(&root, "windows_path", r"C:\dev\oxid").expect("add dependency");
        let manifest = load_manifest(&root.join("oxid.toml")).expect("load manifest");
        assert_eq!(manifest.dependencies.get("windows_path").map(String::as_str), Some(r"C:\dev\oxid"));
        assert_eq!(
            packages::list_dependencies(&root).expect("list dependencies"),
            vec![("windows_path".to_string(), r"C:\dev\oxid".to_string())]
        );
        fs::remove_dir_all(&root).expect("remove temp project");
    }

    #[test]
    fn embedded_compiler_demo_artifact_roundtrip_is_deterministic() {
        let root = unique_temp_dir("bootstrap");
        fs::create_dir_all(&root).expect("temp project");
        bootstrap_project(&root, false).expect("bootstrap parity");
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
