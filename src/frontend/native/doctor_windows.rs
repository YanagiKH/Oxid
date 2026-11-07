//! Bounded Windows probes. A new process joins its owned job atomically, before
//! its first instruction. Local byte pipes use documented synchronous NOWAIT
//! polling: no blocking PeekNamedPipe, read thread or pending OVERLAPPED I/O.
use super::super::{
    resolve_tool, runtime_compile_args, tool, validate_version_output, NativeBuildSettings,
    Workspace, LLVM_VERSION, RUNTIME,
};
use super::{Report, Status};
use std::{
    ffi::{c_void, OsStr, OsString},
    fs,
    io::Read,
    mem::{offset_of, size_of, size_of_val},
    os::windows::{ffi::OsStrExt, process::ExitStatusExt},
    path::Path,
    process::{ExitStatus, Output},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type RawHandle = *mut c_void;
const INVALID_HANDLE: RawHandle = -1isize as RawHandle;
const OUTPUT_LIMIT: usize = 65_536;
const TOOL_TIMEOUT: Duration = Duration::from_secs(5);
const BUILD_TIMEOUT: Duration = Duration::from_secs(20);
const RUN_TIMEOUT: Duration = Duration::from_secs(2);
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(100);
const HANDLE_LIST: usize = 0x0002_0002;
const JOB_LIST: usize = 0x0002_000d;
const STARTF_USESTDHANDLES: u32 = 0x100;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ERROR_PIPE_CONNECTED: u32 = 535;
const ERROR_NO_DATA: u32 = 232;
const ERROR_BROKEN_PIPE: u32 = 109;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
static NEXT_PIPE: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
struct SecurityAttributes {
    bytes: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct StartupInfo {
    bytes: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_chars: u32,
    y_chars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_bytes: u16,
    reserved_data: *mut u8,
    stdin: RawHandle,
    stdout: RawHandle,
    stderr: RawHandle,
}
#[repr(C)]
struct StartupInfoEx {
    startup: StartupInfo,
    attributes: *mut c_void,
}
#[repr(C)]
struct ProcessInformation {
    process: RawHandle,
    thread: RawHandle,
    process_id: u32,
    thread_id: u32,
}
#[repr(C)]
struct JobBasicLimits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    process_limit: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
struct JobLimits {
    basic: JobBasicLimits,
    io_counters: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}
const _: () = {
    assert!(size_of::<SecurityAttributes>() == 24);
    assert!(size_of::<StartupInfo>() == 104);
    assert!(offset_of!(StartupInfo, stdin) == 80);
    assert!(offset_of!(StartupInfo, stderr) == 96);
    assert!(size_of::<StartupInfoEx>() == 112);
    assert!(offset_of!(StartupInfoEx, attributes) == 104);
    assert!(size_of::<ProcessInformation>() == 24);
    assert!(size_of::<JobBasicLimits>() == 64);
    assert!(offset_of!(JobBasicLimits, flags) == 16);
    assert!(size_of::<JobLimits>() == 144);
    assert!(offset_of!(JobLimits, job_memory) == 120);
};

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: RawHandle) -> i32;
    fn GetLastError() -> u32;
    fn CompareStringOrdinal(
        left: *const u16,
        left_length: i32,
        right: *const u16,
        right_length: i32,
        ignore_case: i32,
    ) -> i32;
    fn CreateJobObjectW(attributes: *const SecurityAttributes, name: *const u16) -> RawHandle;
    fn SetInformationJobObject(
        job: RawHandle,
        class: i32,
        information: *const c_void,
        bytes: u32,
    ) -> i32;
    fn TerminateJobObject(job: RawHandle, code: u32) -> i32;
    fn QueryInformationJobObject(
        job: RawHandle,
        class: i32,
        information: *mut c_void,
        bytes: u32,
        returned_bytes: *mut u32,
    ) -> i32;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        bytes: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *mut c_void,
        bytes: usize,
        previous: *mut c_void,
        previous_bytes: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: *const c_void,
        thread_attributes: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        cwd: *const u16,
        startup: *const StartupInfoEx,
        information: *mut ProcessInformation,
    ) -> i32;
    fn CreateNamedPipeW(
        name: *const u16,
        access: u32,
        mode: u32,
        instances: u32,
        output_bytes: u32,
        input_bytes: u32,
        timeout: u32,
        attributes: *const SecurityAttributes,
    ) -> RawHandle;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        attributes: *const c_void,
        disposition: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn ConnectNamedPipe(pipe: RawHandle, overlapped: *mut c_void) -> i32;
    fn ReadFile(
        file: RawHandle,
        bytes: *mut u8,
        length: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: RawHandle, code: *mut u32) -> i32;
}

fn os_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}
struct Handle(RawHandle);
impl Handle {
    fn owned(raw: RawHandle, operation: &str) -> Result<Self, String> {
        if raw.is_null() || raw == INVALID_HANDLE {
            Err(os_error(operation))
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Exactly one owner retains this valid non-null kernel handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Job(Handle);
impl Job {
    fn new() -> Result<Self, String> {
        // SAFETY: An unnamed, non-inheritable job has one owned handle.
        let job = Self(Handle::owned(
            unsafe { CreateJobObjectW(ptr::null(), ptr::null()) },
            "create probe job",
        )?);
        // SAFETY: This C layout consists only of initialized integer fields.
        let mut limits: JobLimits = unsafe { std::mem::zeroed() };
        limits.basic.flags = 0x2000 | 0x200; // KILL_ON_JOB_CLOSE | JOB_MEMORY
        limits.job_memory = 1024 * 1024 * 1024;
        // SAFETY: The complete repr(C) storage is readable for its exact size.
        if unsafe {
            SetInformationJobObject(
                job.0 .0,
                9,
                (&limits as *const JobLimits).cast(),
                size_of::<JobLimits>() as u32,
            )
        } == 0
        {
            return Err(os_error("set bounded probe job limits"));
        }
        Ok(job)
    }
    fn active_processes(&self) -> Result<u32, String> {
        // JOBOBJECT_BASIC_ACCOUNTING_INFORMATION: four LARGE_INTEGER time
        // values, page faults, total processes, active processes, terminated.
        let mut accounting = [0u64; 6];
        // SAFETY: Aligned, initialized 48-byte storage for the documented
        // fixed accounting information. Only bytes 40..44 hold ActiveProcesses.
        if unsafe {
            QueryInformationJobObject(
                self.0 .0,
                1,
                accounting.as_mut_ptr().cast(),
                size_of_val(&accounting) as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(os_error("query owned probe job accounting"));
        }
        Ok(accounting[5] as u32)
    }
    fn terminate(&self) -> Result<(), String> {
        // SAFETY: This unnamed owned job contains only this probe and descendants.
        if unsafe { TerminateJobObject(self.0 .0, 1) } == 0 {
            Err(os_error("terminate probe job"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.terminate();
        // The last job handle closes next. KILL_ON_JOB_CLOSE is an independent
        // cleanup path; it cannot affect an unrelated PID or an inherited job.
    }
}
// Retain the actual process handle and job together through every success and
// failure path. The process identity is never reconstructed from a numeric PID.
struct ProbeProcess {
    process: Handle,
    job: Job,
    cleaned: bool,
}
impl ProbeProcess {
    fn cleanup(&mut self) -> Result<(), String> {
        if self.cleaned {
            return Ok(());
        }
        self.job.terminate()?;
        let deadline = Instant::now()
            .checked_add(CLEANUP_TIMEOUT)
            .ok_or("probe cleanup deadline overflow")?;
        loop {
            let wait = unsafe { WaitForSingleObject(self.process.0, 0) };
            if wait == WAIT_OBJECT_0 && self.job.active_processes()? == 0 {
                self.cleaned = true;
                return Ok(());
            }
            if wait != WAIT_OBJECT_0 && wait != WAIT_TIMEOUT {
                return Err(os_error("wait for terminated probe process"));
            }
            if Instant::now() >= deadline {
                return Err(
                    "probe process tree did not terminate within the cleanup deadline".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for ProbeProcess {
    fn drop(&mut self) {
        let _ = self.cleanup();
        // The job's final close still enforces KILL_ON_JOB_CLOSE if Windows
        // cannot confirm completion. No successful result is returned then.
    }
}
fn wide(value: &OsStr) -> Result<Vec<u16>, String> {
    let mut result = Vec::new();
    for unit in value.encode_wide() {
        if unit == 0 || result.len() >= 32_766 {
            return Err("probe pathname or argument exceeds Windows bounds".into());
        }
        result.push(unit);
    }
    result.push(0);
    Ok(result)
}
struct Pipe {
    reader: Handle,
    writer: Option<Handle>,
    eof: bool,
}
impl Pipe {
    fn new() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid probe clock")?
            .as_nanos();
        let name = wide(OsStr::new(&format!(
            r"\\.\pipe\oxid-native-doctor-{}-{stamp}-{}",
            std::process::id(),
            NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
        )))?;
        // SAFETY: The server is non-inheritable, byte-oriented, local-only and
        // synchronous NOWAIT. FIRST_PIPE_INSTANCE refuses an existing name.
        let reader = Handle::owned(
            unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    1 | 0x0008_0000,
                    1 | 8,
                    1,
                    0,
                    4096,
                    0,
                    ptr::null(),
                )
            },
            "create nonblocking probe pipe",
        )?;
        let attributes = SecurityAttributes {
            bytes: size_of::<SecurityAttributes>() as u32,
            descriptor: ptr::null_mut(),
            inherit: 1,
        };
        // SAFETY: A synchronous, inheritable writer is the only client opened
        // here. Only this writer appears in the explicit child HANDLE_LIST.
        let writer = Handle::owned(
            unsafe {
                CreateFileW(
                    name.as_ptr(),
                    0x4000_0000,
                    0,
                    (&attributes as *const SecurityAttributes).cast(),
                    3,
                    0,
                    ptr::null_mut(),
                )
            },
            "connect probe output writer",
        )?;
        // NOWAIT TRUE means listening, not connected. With our real client
        // already open, require the documented FALSE/PIPE_CONNECTED state.
        let connected = unsafe { ConnectNamedPipe(reader.0, ptr::null_mut()) };
        if connected != 0 || unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            return Err("probe pipe did not confirm an existing client connection".into());
        }
        Ok(Self {
            reader,
            writer: Some(writer),
            eof: false,
        })
    }
    fn drain(&mut self, output: &mut Vec<u8>, deadline: Instant) -> Result<(), String> {
        let mut buffer = [0u8; 4096];
        while !self.eof {
            if Instant::now() >= deadline {
                return Err("probe capture timed out".into());
            }
            let mut count = 0;
            // SAFETY: This server handle uses PIPE_NOWAIT. ReadFile returns
            // immediately on an empty pipe, even in a multithreaded caller.
            // No asynchronous I/O or borrowed buffer survives this call.
            let result = unsafe {
                ReadFile(
                    self.reader.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut count,
                    ptr::null_mut(),
                )
            };
            if result == 0 {
                match unsafe { GetLastError() } {
                    ERROR_NO_DATA => return Ok(()),
                    ERROR_BROKEN_PIPE => {
                        self.eof = true;
                        return Ok(());
                    }
                    _ => return Err(os_error("read nonblocking probe output")),
                }
            }
            if count == 0 {
                return Ok(());
            }
            let count = count as usize;
            if count > buffer.len() || count > OUTPUT_LIMIT.saturating_sub(output.len()) {
                return Err("probe output exceeded 65536 bytes per stream".into());
            }
            output
                .try_reserve_exact(count)
                .map_err(|_| "cannot reserve bounded probe output")?;
            output.extend_from_slice(&buffer[..count]);
        }
        Ok(())
    }
}
#[repr(align(16))]
struct AttributeStorage([u8; 4096]);
struct Attributes<'a>(&'a mut AttributeStorage);
impl<'a> Attributes<'a> {
    fn new(storage: &'a mut AttributeStorage) -> Result<Self, String> {
        let mut bytes = 0;
        // SAFETY: The first call only obtains the size for two fixed attributes.
        let result =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes) };
        if result != 0 || unsafe { GetLastError() } != 122 || bytes == 0 || bytes > storage.0.len()
        {
            return Err("cannot size bounded Windows probe attributes".into());
        }
        if unsafe {
            InitializeProcThreadAttributeList(storage.0.as_mut_ptr().cast(), 2, 0, &mut bytes)
        } == 0
        {
            return Err(os_error("initialize probe attributes"));
        }
        Ok(Self(storage))
    }
    fn raw(&mut self) -> *mut c_void {
        self.0 .0.as_mut_ptr().cast()
    }
    fn set(&mut self, attribute: usize, values: &mut [RawHandle]) -> Result<(), String> {
        // SAFETY: Handle arrays remain alive through CreateProcessW and deletion
        // of this attribute list; neither list contains a pseudo handle.
        if unsafe {
            UpdateProcThreadAttribute(
                self.raw(),
                0,
                attribute,
                values.as_mut_ptr().cast(),
                std::mem::size_of_val(values),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(os_error("set atomic probe process attributes"));
        }
        Ok(())
    }
}
impl Drop for Attributes<'_> {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.raw());
        }
    }
}
fn command_line(program: &Path, arguments: &[OsString]) -> Result<Vec<u16>, String> {
    fn append(line: &mut Vec<u16>, unit: u16, count: usize) -> Result<(), String> {
        if count > 32_766usize.saturating_sub(line.len()) {
            return Err("probe command exceeds Windows bounds".into());
        }
        line.try_reserve_exact(count)
            .map_err(|_| "cannot reserve bounded probe command")?;
        line.extend(std::iter::repeat_n(unit, count));
        Ok(())
    }
    let mut line = Vec::new();
    for argument in
        std::iter::once(program.as_os_str()).chain(arguments.iter().map(OsString::as_os_str))
    {
        if !line.is_empty() {
            append(&mut line, u16::from(b' '), 1)?;
        }
        append(&mut line, u16::from(b'"'), 1)?;
        let mut slashes = 0;
        for unit in argument.encode_wide() {
            if unit == 0 {
                return Err("NUL in probe argument".into());
            }
            if unit == u16::from(b'\\') {
                // Reject before counting or allocating an arbitrarily long run.
                if slashes >= 32_766 {
                    return Err("probe command exceeds Windows bounds".into());
                }
                slashes += 1;
                continue;
            }
            append(
                &mut line,
                u16::from(b'\\'),
                slashes * if unit == u16::from(b'"') { 2 } else { 1 },
            )?;
            slashes = 0;
            if unit == u16::from(b'"') {
                append(&mut line, u16::from(b'\\'), 1)?;
            }
            append(&mut line, unit, 1)?;
        }
        append(&mut line, u16::from(b'\\'), slashes * 2)?;
        append(&mut line, u16::from(b'"'), 1)?;
    }
    line.push(0);
    Ok(line)
}
fn environment(deadline: Instant) -> Result<Vec<u16>, String> {
    environment_block(std::env::vars_os(), deadline)
}
fn environment_block(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
    deadline: Instant,
) -> Result<Vec<u16>, String> {
    // Environment ordering is Windows ordinal, case-insensitive Unicode by
    // variable name. A fallible in-place heap sort can stop immediately on an
    // API error or deadline; it never substitutes a different collation.
    type Entry = (usize, Vec<u16>);
    fn compare(
        left: &Entry,
        right: &Entry,
        deadline: Instant,
    ) -> Result<std::cmp::Ordering, String> {
        if Instant::now() >= deadline {
            return Err("probe environment setup timed out".into());
        }
        // SAFETY: The lengths are bounded UTF-16 key lengths in these live
        // entries, without their separators/values. Windows retains raw UTF-16.
        match unsafe {
            CompareStringOrdinal(
                left.1.as_ptr(),
                left.0 as i32,
                right.1.as_ptr(),
                right.0 as i32,
                1,
            )
        } {
            1 => Ok(std::cmp::Ordering::Less),
            2 => Ok(std::cmp::Ordering::Equal),
            3 => Ok(std::cmp::Ordering::Greater),
            _ => Err(os_error("compare Windows probe environment names")),
        }
    }
    fn sift(entries: &mut [Entry], mut root: usize, deadline: Instant) -> Result<(), String> {
        while root < entries.len() / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < entries.len()
                && compare(&entries[child], &entries[child + 1], deadline)?.is_lt()
            {
                child += 1;
            }
            if !compare(&entries[root], &entries[child], deadline)?.is_lt() {
                break;
            }
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    let mut entries = Vec::new();
    let mut units = 1usize;
    for (key, value) in variables {
        if Instant::now() >= deadline {
            return Err("probe environment setup timed out".into());
        }
        if key
            .to_string_lossy()
            .eq_ignore_ascii_case("CCC_OVERRIDE_OPTIONS")
        {
            continue;
        }
        let mut entry = wide(&key)?;
        entry.pop();
        let key_units = entry.len();
        entry.push(u16::from(b'='));
        entry.extend(wide(&value)?);
        units = units
            .checked_add(entry.len())
            .ok_or("probe environment overflow")?;
        if units > 2 * 1024 * 1024 {
            return Err("probe environment exceeds UTF-16 storage limit".into());
        }
        entries.push((key_units, entry));
    }
    for root in (0..entries.len() / 2).rev() {
        sift(&mut entries, root, deadline)?;
    }
    for end in (1..entries.len()).rev() {
        entries.swap(0, end);
        sift(&mut entries[..end], 0, deadline)?;
    }
    let mut block = Vec::new();
    for (_, entry) in entries {
        block.extend(entry);
    }
    block.push(0);
    if block.len() == 1 {
        block.push(0);
    }
    Ok(block)
}
fn capture(
    program: &Path,
    arguments: &[OsString],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<Output, String> {
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .ok_or("probe timeout overflow")?;
    let job = Job::new()?;
    let mut stdout = Pipe::new()?;
    let mut stderr = Pipe::new()?;
    let attributes = SecurityAttributes {
        bytes: size_of::<SecurityAttributes>() as u32,
        descriptor: ptr::null_mut(),
        inherit: 1,
    };
    let nul = wide(OsStr::new("NUL"))?;
    // SAFETY: NUL input is synchronous and inheritable only through HANDLE_LIST.
    let input = Handle::owned(
        unsafe {
            CreateFileW(
                nul.as_ptr(),
                0x8000_0000,
                3,
                (&attributes as *const SecurityAttributes).cast(),
                3,
                0,
                ptr::null_mut(),
            )
        },
        "open probe NUL input",
    )?;
    let mut handles = [
        input.0,
        stdout.writer.as_ref().unwrap().0,
        stderr.writer.as_ref().unwrap().0,
    ];
    let mut jobs = [job.0 .0];
    let mut storage = AttributeStorage([0; 4096]);
    let mut attributes = Attributes::new(&mut storage)?;
    attributes.set(HANDLE_LIST, &mut handles)?;
    attributes.set(JOB_LIST, &mut jobs)?;
    let application = wide(program.as_os_str())?;
    let mut line = command_line(program, arguments)?;
    let directory = cwd.map(|path| wide(path.as_os_str())).transpose()?;
    let environment = environment(deadline)?;
    // SAFETY: These C structures contain only scalar/pointer fields. All values
    // referenced by attributes, startup, argv and environment outlive the call.
    let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
    startup.startup.bytes = size_of::<StartupInfoEx>() as u32;
    startup.startup.flags = STARTF_USESTDHANDLES;
    startup.startup.stdin = handles[0];
    startup.startup.stdout = handles[1];
    startup.startup.stderr = handles[2];
    startup.attributes = attributes.raw();
    let mut information: ProcessInformation = unsafe { std::mem::zeroed() };
    if Instant::now() >= deadline {
        return Err("probe setup timed out before process creation".into());
    }
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
            environment.as_ptr().cast(),
            directory.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
            &startup,
            &mut information,
        )
    };
    if created == 0 {
        return Err(os_error("create atomically job-bound probe (Windows 10 and a compatible containing job are required)"));
    }
    let process = Handle::owned(information.process, "own probe process")?;
    let mut child = ProbeProcess {
        process,
        job,
        cleaned: false,
    };
    let thread = Handle::owned(information.thread, "own probe thread")?;
    drop(thread);
    drop(attributes);
    stdout.writer.take();
    stderr.writer.take();
    drop(input);
    let (mut output, mut errors) = (Vec::new(), Vec::new());
    loop {
        stdout.drain(&mut output, deadline)?;
        stderr.drain(&mut errors, deadline)?;
        let wait = unsafe { WaitForSingleObject(child.process.0, 0) };
        if wait == WAIT_OBJECT_0 {
            let mut code = 0;
            if unsafe { GetExitCodeProcess(child.process.0, &mut code) } == 0 {
                return Err(os_error("read probe process status"));
            }
            child.cleanup()?;
            let cleanup = Instant::now()
                .checked_add(CLEANUP_TIMEOUT)
                .ok_or("probe cleanup overflow")?;
            while !stdout.eof || !stderr.eof {
                stdout.drain(&mut output, cleanup)?;
                stderr.drain(&mut errors, cleanup)?;
                if stdout.eof && stderr.eof {
                    break;
                }
                if Instant::now() >= cleanup {
                    return Err("probe output handles did not close after job termination".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let status = ExitStatus::from_raw(code);
            if !status.success() {
                let detail = if errors.is_empty() { &output } else { &errors };
                return Err(format!(
                    "{} failed ({status}): {}",
                    program.display(),
                    super::clipped(&String::from_utf8_lossy(detail))
                ));
            }
            return Ok(Output {
                status,
                stdout: output,
                stderr: errors,
            });
        }
        if wait != WAIT_TIMEOUT {
            return Err(os_error("wait for owned probe process"));
        }
        if Instant::now() >= deadline {
            child.cleanup()?;
            return Err(format!(
                "probe timed out after {} seconds",
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn artifact(path: &Path, executable: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("probe artifact must be a regular file".into());
    }
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    if !executable {
        let mut header = [0u8; 20];
        file.read_exact(&mut header)
            .map_err(|error| error.to_string())?;
        if u16::from_le_bytes(header[..2].try_into().unwrap()) != 0x8664
            || u16::from_le_bytes(header[16..18].try_into().unwrap()) != 0
        {
            return Err("probe object is not AMD64 COFF".into());
        }
        return Ok(());
    }
    use std::io::{Seek, SeekFrom};
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|error| error.to_string())?;
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if &dos[..2] != b"MZ" || offset > 1024 * 1024 || offset + 144 > metadata.len() {
        return Err("probe executable has an invalid PE header".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| error.to_string())?;
    let mut pe = [0u8; 144];
    file.read_exact(&mut pe)
        .map_err(|error| error.to_string())?;
    let dll_flags = u16::from_le_bytes(pe[94..96].try_into().unwrap());
    if &pe[..4] != b"PE\0\0"
        || u16::from_le_bytes(pe[4..6].try_into().unwrap()) != 0x8664
        || u16::from_le_bytes(pe[24..26].try_into().unwrap()) != 0x20b
        || dll_flags & 0x160 != 0x160
        || u64::from_le_bytes(pe[96..104].try_into().unwrap()) != 8 * 1024 * 1024
    {
        return Err(
            "probe executable is not MSVC AMD64 PE32+ with ASLR, NX and an 8 MiB stack".into(),
        );
    }
    Ok(())
}

pub(super) fn probe(report: &mut Report) {
    let workspace = match fs::canonicalize(std::env::temp_dir())
        .map_err(|error| error.to_string())
        .and_then(|parent| Workspace::create(&parent).map_err(|error| error.message))
    {
        Ok(workspace) => workspace,
        Err(error) => {
            report.set("temporary-directory", Err(error));
            return;
        }
    };
    report.set(
        "temporary-directory",
        Ok("private temporary probe storage available".into()),
    );
    let mut clang = None;
    for (id, name, marker) in [
        ("clang", "clang", "clang version"),
        ("opt", "opt", "LLVM version"),
        ("linker", NativeBuildSettings::linker(), "LLD"),
    ] {
        let result = (|| {
            let path = resolve_tool(tool(name)).map_err(|error| error.message)?;
            let output = capture(
                &path,
                &[OsString::from("--version")],
                Some(&workspace.0),
                TOOL_TIMEOUT,
            )?;
            validate_version_output(&path, marker, &output.stdout)
                .map_err(|error| error.message)?;
            if id == "clang" {
                clang = Some(path.clone());
            }
            Ok(format!("{}; LLVM {LLVM_VERSION}", path.display()))
        })();
        report.set(id, result);
    }
    if report
        .checks
        .iter()
        .any(|check| check.status == Status::Failed)
    {
        return;
    }
    let sdk = (|| {
        fs::write(workspace.0.join("runtime.c"), RUNTIME).map_err(|error| error.to_string())?;
        capture(
            clang.as_ref().unwrap(),
            &runtime_compile_args(&NativeBuildSettings::new(false)),
            Some(&workspace.0),
            TOOL_TIMEOUT,
        )?;
        artifact(
            &workspace.0.join(NativeBuildSettings::runtime_object()),
            false,
        )?;
        Ok("production LLP64 C runtime compiled against the Windows SDK to AMD64 COFF".into())
    })();
    let sdk_ok = sdk.is_ok();
    report.set("sdk", sdk);
    if !sdk_ok {
        return;
    }
    let executable = workspace.0.join("probe.exe");
    let linked = (|| {
        let compiler = std::env::current_exe().map_err(|error| error.to_string())?;
        let source = workspace.0.join("probe.ox");
        fs::write(&source, "fn main()->i32{return 42;}\n").map_err(|error| error.to_string())?;
        let arguments = [
            OsString::from("compile"),
            source.into_os_string(),
            OsString::from("--edition=typed-preview"),
            OsString::from("--backend=llvm"),
            OsString::from("--message-format=json"),
            OsString::from("--output"),
            executable.as_os_str().to_owned(),
        ];
        capture(&compiler, &arguments, None, BUILD_TIMEOUT)?;
        artifact(&executable, true)?;
        Ok("production compiler linked a fixed MSVC AMD64 PE32+ program with static CRT, ASLR, NX and an 8 MiB stack".into())
    })();
    let linked_ok = linked.is_ok();
    report.set("compile-link", linked);
    if !linked_ok {
        return;
    }
    report.set("execute", capture(&executable, &[], Some(&workspace.0), RUN_TIMEOUT).and_then(|output| {
        if output.stdout == b"42\n" && output.stderr.is_empty() { Ok("fixed PE program exited 0 with exact stdout 42\\n".into()) }
        else { Err("native probe returned unexpected stdout or stderr; expected exactly 42 followed by a newline".into()) }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn powershell(script: &str, timeout: Duration) -> Result<Output, String> {
        let system = std::env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?;
        capture(
            &Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            &[
                OsString::from("-NoLogo"),
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from(script),
            ],
            None,
            timeout,
        )
    }

    #[test]
    fn byte_pipe_capture_remains_nonblocking_in_a_multithreaded_process() {
        let result = std::thread::scope(|scope| {
            let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(0);
            let (release_sender, release_receiver) = std::sync::mpsc::channel::<()>();
            let background = scope.spawn(move || {
                ready_sender.send(()).unwrap();
                let _ = release_receiver.recv();
            });
            // Keep another thread alive through setup and cold child startup.
            // Channel drop and scoped joining also release it during unwinding.
            ready_receiver.recv_timeout(RUN_TIMEOUT).unwrap();
            assert!(!background.is_finished());
            let result = powershell(
                "[Console]::OpenStandardOutput().WriteByte(26)",
                TOOL_TIMEOUT,
            );
            assert!(!background.is_finished());
            drop(release_sender);
            background.join().unwrap();
            result
        });
        let output = result.unwrap();
        assert_eq!(output.stdout, [26]);
        assert!(output.stderr.is_empty(), "{output:?}");
    }

    #[test]
    fn capture_keeps_the_inclusive_per_stream_limit() {
        let output = powershell(
            "$b = New-Object byte[] 65536; [Console]::OpenStandardOutput().Write($b,0,$b.Length)",
            TOOL_TIMEOUT,
        )
        .unwrap();
        assert_eq!(output.stdout, vec![0; OUTPUT_LIMIT]);
        assert!(output.stderr.is_empty());
        for stream in ["Output", "Error"] {
            let error = powershell(
                &format!("$b = New-Object byte[] 65537; [Console]::OpenStandard{stream}().Write($b,0,$b.Length)"),
                TOOL_TIMEOUT,
            )
            .unwrap_err();
            assert!(error.contains("exceeded 65536"), "{error}");
        }
    }

    #[test]
    fn a_sleeping_probe_has_a_hard_deadline_and_no_success_result() {
        let started = Instant::now();
        let error = powershell("Start-Sleep -Seconds 30", Duration::from_millis(100)).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(started.elapsed() < RUN_TIMEOUT, "{error}");
    }

    #[test]
    fn environment_uses_unicode_ordinal_names_and_keeps_raw_values() {
        let values: Vec<(OsString, OsString)> = [
            ("Ö_b", "second"),
            ("ä_a", "first=雪"),
            ("ccc_override_options", "must not enter the tool"),
            ("INCLUDE", r"C:\Program Files\SDK;C:\VC\include"),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();
        let block = environment_block(values, Instant::now() + TOOL_TIMEOUT).unwrap();
        let expected: Vec<u16> =
            "INCLUDE=C:\\Program Files\\SDK;C:\\VC\\include\0ä_a=first=雪\0Ö_b=second\0\0"
                .encode_utf16()
                .collect();
        assert_eq!(block, expected);
        assert!(
            environment_block([(OsString::from("X"), OsString::from("Y"))], Instant::now())
                .is_err()
        );
    }

    #[test]
    fn process_arguments_keep_empty_unicode_quotes_and_trailing_backslashes() {
        #[link(name = "Shell32")]
        unsafe extern "system" {
            fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
        }
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn LocalFree(memory: *mut c_void) -> *mut c_void;
        }
        let arguments: Vec<OsString> = [
            "",
            "遷移 Oxid",
            "one\\two\\",
            "a\\\"b",
            "\"quoted\"",
            "\\\\",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let program = Path::new(r"C:\Program Files\Oxid\oxid.exe");
        let command = command_line(program, &arguments).unwrap();
        let mut count = 0;
        let parsed = unsafe { CommandLineToArgvW(command.as_ptr(), &mut count) };
        assert!(!parsed.is_null());
        let actual = unsafe { std::slice::from_raw_parts(parsed, count as usize) };
        let expected =
            std::iter::once(program.as_os_str()).chain(arguments.iter().map(OsString::as_os_str));
        assert_eq!(actual.len(), arguments.len() + 1);
        for (actual, expected) in actual.iter().zip(expected) {
            let mut length = 0;
            while unsafe { *actual.add(length) } != 0 {
                length += 1;
            }
            assert_eq!(
                unsafe { std::slice::from_raw_parts(*actual, length) },
                expected.encode_wide().collect::<Vec<_>>()
            );
        }
        unsafe {
            LocalFree(parsed.cast());
        }
        assert!(command_line(program, &[OsString::from("\\".repeat(32_767))]).is_err());
    }
}
