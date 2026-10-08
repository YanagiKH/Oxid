//! Bounded, nonblocking full-duplex lexical-provider transport on Linux.
//! Process lifetime control is not an OS sandbox or a child-RSS limit.
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            process::{CommandExt, ExitStatusExt},
        },
    },
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Fixed named transport carriers; stdout's actual allocation is separate.
/// Standard-library Command internals and OS pipe/process memory are not a
/// claim about allocator usage or child RSS.
pub(super) fn named_bytes() -> usize {
    std::mem::size_of::<Capture>()
        + std::mem::size_of::<ChildGroup>()
        + std::mem::size_of::<Command>()
        + std::mem::size_of::<Option<std::process::ChildStdin>>()
        + std::mem::size_of::<std::process::ChildStdout>()
        + std::mem::size_of::<std::process::ChildStderr>()
        + std::mem::size_of::<libc::siginfo_t>()
        + 3 * std::mem::size_of::<Instant>()
        + 16 * std::mem::size_of::<usize>()
        + 42 // executable descriptor spelling and reversed decimal digits
}

pub(super) const OUTPUT_MAX: usize = 2_133_666;
pub(super) const STDERR_MAX: usize = 4096;
pub(super) const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stop {
    Exited,
    Spawn,
    Io,
    Timeout,
    StdoutLimit,
    StderrLimit,
    OutputBudget,
    Allocation,
}
impl Stop {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::Spawn => "spawn-failed",
            Self::Io => "transport-failed",
            Self::Timeout => "deadline",
            Self::StdoutLimit => "stdout-limit",
            Self::StderrLimit => "stderr-limit",
            Self::OutputBudget => "output-budget",
            Self::Allocation => "allocation-failed",
        }
    }
}

pub(super) struct Capture {
    /// Length is bytes actually received; capacity is the checked cap plus one.
    /// Even an unexpected allocator over-reservation remains visible here.
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: [u8; STDERR_MAX + 1],
    pub(super) stderr_len: usize,
    pub(super) input_written: usize,
    pub(super) stdin_closed: bool,
    pub(super) stdout_eof: bool,
    pub(super) stderr_eof: bool,
    pub(super) status: Option<ExitStatus>,
    pub(super) stop: Stop,
    pub(super) spawned: bool,
}
impl Capture {
    fn empty() -> Self {
        Self {
            stdout: Vec::new(),
            stderr: [0; STDERR_MAX + 1],
            stderr_len: 0,
            input_written: 0,
            stdin_closed: false,
            stdout_eof: false,
            stderr_eof: false,
            status: None,
            stop: Stop::Spawn,
            spawned: false,
        }
    }
    fn allocate_stdout(&mut self, reserved: usize) -> bool {
        if self.stdout.try_reserve_exact(reserved).is_err() || self.stdout.capacity() != reserved {
            self.stop = Stop::Allocation;
            return false;
        }
        self.stdout.resize(reserved, 0);
        true
    }
    pub(super) fn code(&self) -> Option<i32> {
        self.status.and_then(|status| status.code())
    }
    pub(super) fn signal(&self) -> Option<i32> {
        self.status.and_then(|status| status.signal())
    }
}

struct ChildGroup {
    child: Child,
    finished: bool,
}
impl ChildGroup {
    fn finish(&mut self) -> Option<ExitStatus> {
        // WNOWAIT retained the leader PID. Kill both its original group and
        // the owned leader individually; never rely solely on group membership.
        // SAFETY: this is our unreaped child PID, not the caller's group.
        unsafe {
            libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.child.kill();
        self.finished = true;
        let cleanup_started = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => (),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
                Err(_) => return None,
            }
            if cleanup_started.elapsed() >= Duration::from_millis(500) {
                // Do not turn a failed/undeliverable kill into an unbounded
                // wait. The caller reports missing leader-reap evidence as a
                // failed contract. Kernel-stalled processes are not concealed.
                return None;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for ChildGroup {
    fn drop(&mut self) {
        if !self.finished {
            self.finish();
        }
    }
}

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    // SAFETY: fd is a live owned child pipe. Only its status flags are changed;
    // ownership, descriptor lifetime and inherited process policy are untouched.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn exited(child: &Child) -> io::Result<bool> {
    // SAFETY: zero is a valid empty siginfo value; waitid fills this caller-owned
    // value. WNOWAIT observes only our child and does not reap/release its PID.
    let mut information: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            &mut information,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { information.si_pid() } != 0)
}

#[derive(Clone, Copy)]
enum Drain {
    Pending,
    Eof,
    Deadline,
}

fn drain(
    reader: &mut impl Read,
    bytes: &mut [u8],
    used: &mut usize,
    started: Instant,
    deadline: Duration,
) -> io::Result<Drain> {
    // Fairness keeps a continuously readable stdout from starving input or
    // stderr. Check time inside the loop, including repeated interruptions.
    let quantum_end = bytes.len().min(used.saturating_add(64 * 1024));
    while *used < quantum_end {
        if started.elapsed() >= deadline {
            return Ok(Drain::Deadline);
        }
        match reader.read(&mut bytes[*used..quantum_end]) {
            Ok(0) => return Ok(Drain::Eof),
            Ok(count) => *used += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(Drain::Pending),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(Drain::Pending)
}

/// File descriptors must remain alive throughout the call. Bundle guarantees
/// descriptors outside 0..=2, immutable executable bytes, and a private cwd.
pub(super) fn run(
    program: &File,
    directory: &File,
    input: &[u8],
    output_limit: usize,
    deadline: Duration,
) -> Capture {
    let mut captured = Capture::empty();
    if output_limit > OUTPUT_MAX {
        captured.stop = Stop::OutputBudget;
        return captured;
    }
    let reserved = output_limit + 1;
    if !captured.allocate_stdout(reserved) {
        return captured;
    }
    // No later stdout operation grows this allocation. Truncation below retains
    // its actual capacity for the coordinator's named-storage accounting.
    let mut stdout_len = 0;
    let started = Instant::now();
    let program_fd = program.as_raw_fd();
    let directory_fd = directory.as_raw_fd();
    if program_fd < 3 || directory_fd < 3 {
        captured.stdout.clear();
        return captured;
    }
    // Avoid constructing a second dynamically sized executable-path carrier.
    let mut path = [0u8; 32];
    let prefix = b"/proc/self/fd/";
    path[..prefix.len()].copy_from_slice(prefix);
    let mut reversed = [0u8; 10];
    let mut digits = 0;
    let mut value = program_fd as u32;
    loop {
        reversed[digits] = b'0' + (value % 10) as u8;
        digits += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..digits {
        path[prefix.len() + index] = reversed[digits - 1 - index];
    }
    let mut command = Command::new(OsStr::from_bytes(&path[..prefix.len() + digits]));
    command
        .arg0("lexer")
        .env_clear()
        .env("PATH", "/no-tools")
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: this closure uses only async-signal-safe Linux syscalls; both
    // borrowed File descriptors stay alive until spawn has completed.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(directory_fd) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
    let Ok(child) = command.spawn() else {
        captured.stdout.clear();
        return captured;
    };
    let mut group = ChildGroup {
        child,
        finished: false,
    };
    captured.spawned = true;
    captured.stop = Stop::Io;
    let mut stdin = group.child.stdin.take();
    let mut stdout = group.child.stdout.take().expect("piped stdout");
    let mut stderr = group.child.stderr.take().expect("piped stderr");
    if nonblocking(stdin.as_ref().expect("piped stdin")).is_err()
        || nonblocking(&stdout).is_err()
        || nonblocking(&stderr).is_err()
    {
        stdin.take();
        captured.stdin_closed = true;
        captured.status = group.finish();
        captured.stdout.clear();
        return captured;
    }
    loop {
        if started.elapsed() >= deadline {
            captured.stop = Stop::Timeout;
            break;
        }
        if captured.input_written == input.len() {
            stdin.take();
            captured.stdin_closed = true;
        }
        if let Some(pipe) = &mut stdin {
            match pipe.write(&input[captured.input_written..]) {
                Ok(0) => break,
                Ok(count) => captured.input_written += count,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => break,
            }
        }
        if !captured.stdout_eof {
            match drain(
                &mut stdout,
                &mut captured.stdout,
                &mut stdout_len,
                started,
                deadline,
            ) {
                Ok(Drain::Eof) => captured.stdout_eof = true,
                Ok(Drain::Pending) => (),
                Ok(Drain::Deadline) => {
                    captured.stop = Stop::Timeout;
                    break;
                }
                Err(_) => break,
            }
        }
        if stdout_len > output_limit {
            captured.stop = Stop::StdoutLimit;
            break;
        }
        if !captured.stderr_eof {
            match drain(
                &mut stderr,
                &mut captured.stderr,
                &mut captured.stderr_len,
                started,
                deadline,
            ) {
                Ok(Drain::Eof) => captured.stderr_eof = true,
                Ok(Drain::Pending) => (),
                Ok(Drain::Deadline) => {
                    captured.stop = Stop::Timeout;
                    break;
                }
                Err(_) => break,
            }
        }
        if captured.stderr_len > STDERR_MAX {
            captured.stop = Stop::StderrLimit;
            break;
        }
        match exited(&group.child) {
            Ok(true)
                if captured.stdout_eof
                    && captured.stderr_eof
                    && captured.input_written == input.len()
                    && captured.stdin_closed =>
            {
                captured.stop = Stop::Exited;
                break;
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(_) => break,
        }
        thread::sleep(Duration::from_millis(1));
    }
    stdin.take();
    captured.stdin_closed = true;
    captured.status = group.finish();
    if captured.stop == Stop::Exited {
        // EOF is checked again after the leader is reaped and ordinary group
        // descendants are killed. An earlier EOF alone never admits output.
        let final_out = drain(
            &mut stdout,
            &mut captured.stdout,
            &mut stdout_len,
            started,
            deadline,
        );
        let final_err = drain(
            &mut stderr,
            &mut captured.stderr,
            &mut captured.stderr_len,
            started,
            deadline,
        );
        captured.stdout_eof = matches!(final_out, Ok(Drain::Eof));
        captured.stderr_eof = matches!(final_err, Ok(Drain::Eof));
        if stdout_len > output_limit {
            captured.stop = Stop::StdoutLimit;
        } else if captured.stderr_len > STDERR_MAX {
            captured.stop = Stop::StderrLimit;
        } else if matches!(final_out, Ok(Drain::Deadline))
            || matches!(final_err, Ok(Drain::Deadline))
        {
            captured.stop = Stop::Timeout;
        } else if !captured.stdout_eof || !captured.stderr_eof || captured.status.is_none() {
            captured.stop = Stop::Io;
        }
    }
    captured.stdout.truncate(stdout_len);
    captured
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(mode: i32) -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-lexical-supervisor-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("mode"), mode.to_string()).unwrap();
            fs::write(path.join("helper.c"), r#"
#include <unistd.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
int main(void) {
  FILE *mode_file=fopen("mode","r"); int mode=-1;
  if (!mode_file || fscanf(mode_file,"%d",&mode)!=1) return 80;
  fclose(mode_file);
  if (mode==7) { FILE*f=fopen("static","w"); if(!f)return 85; fputs("replaced cwd executable",f); fclose(f); mode=0; }
  if (mode==0) { char b; while(read(0,&b,1)==1) { if(write(1,&b,1)!=1)return 81; usleep(1000); } return 0; }
  if (mode==1 || mode==2) { char bytes[256]={0}; for(;;) { if(write(mode,bytes,sizeof bytes)<0)return 82; } }
  if (mode==3) { for(;;)pause(); }
  if (mode==4) { int result=setpgid(0,getpgid(getppid())); FILE*f=fopen("join-result","w"); fprintf(f,"%d",result); fclose(f); for(;;)pause(); }
  if (mode==5 || mode==6) {
    pid_t pid=fork(); if(pid<0)return 83;
    if(pid==0) { FILE*f=fopen("descendant.pid","w"); fprintf(f,"%d",getpid()); fclose(f); close(0); if(mode==5){close(1);close(2);} for(;;)pause(); }
    while(access("descendant.pid",F_OK))usleep(1000);
    return 0;
  }
  if (mode==8) {
    char bytes[4096]; for(size_t i=0;i<sizeof bytes;i++)bytes[i]='o';
    for(int i=0;i<32;i++)if(write(1,bytes,sizeof bytes)!=sizeof bytes)return 86;
    if(write(2,bytes,sizeof bytes)!=sizeof bytes)return 87;
    size_t count=0; ssize_t got;
    while((got=read(0,bytes,sizeof bytes))>0)count+=(size_t)got;
    return count==131072?0:88;
  }
  if (mode==9) { close(0); close(1); close(2); return 0; }
  if (mode==10) { close(1); close(2); for(;;)pause(); }
  return 84;
}
"#).unwrap();
            let output = Command::new("cc")
                .args(["-O0", "helper.c", "-o", "helper"])
                .current_dir(&path)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            Self(path)
        }
        fn run(&self, input: &[u8], cap: usize, deadline: Duration) -> Capture {
            let program = File::open(self.0.join("helper")).unwrap();
            let directory = File::open(&self.0).unwrap();
            run(&program, &directory, input, cap, deadline)
        }
        fn descendant_dead(&self) {
            let pid: i32 = fs::read_to_string(self.0.join("descendant.pid"))
                .unwrap()
                .parse()
                .unwrap();
            for _ in 0..100 {
                match fs::read_to_string(format!("/proc/{pid}/stat")) {
                    Ok(state) if state.split_once(") ").unwrap().1.starts_with('Z') => return,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                    _ => thread::sleep(Duration::from_millis(1)),
                }
            }
            // A failed test must not leave its own synthetic child running.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            panic!("ordinary process-group descendant survived cleanup");
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lexical_supervisor_fragmented_stdio_is_exact_and_leader_reaped() {
        let f = Fixture::new(0);
        let output = f.run(b"hello", 5, DEADLINE);
        assert_eq!(output.stop, Stop::Exited);
        assert_eq!(output.code(), Some(0));
        assert_eq!(&output.stdout, b"hello");
        assert_eq!(output.stderr_len, 0);
        assert_eq!(output.input_written, 5);
        assert!(output.stdin_closed && output.stdout_eof && output.stderr_eof);
        assert_eq!(output.stdout.capacity(), 6);
    }
    #[test]
    fn lexical_supervisor_stream_caps_use_one_witness_and_fail_closed() {
        let output = Fixture::new(1).run(b"", 8, DEADLINE);
        assert_eq!(output.stop, Stop::StdoutLimit);
        assert_eq!(output.stdout.len(), 9);
        assert!(output.status.is_some());
        let output = Fixture::new(2).run(b"", 8, DEADLINE);
        assert_eq!(output.stop, Stop::StderrLimit);
        assert_eq!(output.stderr_len, STDERR_MAX + 1);
        assert!(output.status.is_some());
    }
    #[test]
    fn lexical_supervisor_timeout_cannot_be_defeated_by_joining_parent_group() {
        for mode in [3, 4] {
            let f = Fixture::new(mode);
            let started = Instant::now();
            let output = f.run(b"", 0, Duration::from_millis(250));
            assert_eq!(output.stop, Stop::Timeout);
            assert_eq!(output.signal(), Some(libc::SIGKILL));
            assert!(started.elapsed() < Duration::from_secs(2));
            if mode == 4 {
                assert_eq!(fs::read_to_string(f.0.join("join-result")).unwrap(), "-1");
            }
        }
    }
    #[test]
    fn lexical_supervisor_kills_group_descendants_after_success_and_timeout() {
        for mode in [5, 6] {
            let f = Fixture::new(mode);
            let output = f.run(b"", 0, Duration::from_millis(250));
            assert_eq!(
                output.stop,
                if mode == 5 {
                    Stop::Exited
                } else {
                    Stop::Timeout
                }
            );
            assert_eq!(output.code(), Some(0));
            f.descendant_dead();
        }
    }

    #[test]
    fn lexical_supervisor_full_duplex_stderr_boundary_and_large_input() {
        let f = Fixture::new(8);
        let input = [b'i'; 131072];
        let output = f.run(&input, 131072, DEADLINE);
        assert_eq!(output.stop, Stop::Exited);
        assert_eq!(output.code(), Some(0));
        assert_eq!(output.input_written, input.len());
        assert!(output.stdin_closed && output.stdout_eof && output.stderr_eof);
        assert_eq!(output.stdout.len(), 131072);
        assert_eq!(output.stdout.capacity(), 131073);
        assert!(output.stdout.iter().all(|byte| *byte == b'o'));
        assert_eq!(output.stderr_len, STDERR_MAX);
    }
    #[test]
    fn lexical_supervisor_empty_input_zero_cap_and_exact_maximum_reservation() {
        let f = Fixture::new(0);
        for cap in [0, OUTPUT_MAX] {
            let output = f.run(b"", cap, DEADLINE);
            assert_eq!(output.stop, Stop::Exited);
            assert_eq!(output.code(), Some(0));
            assert_eq!(output.stdout.capacity(), cap + 1);
            assert!(output.stdout.is_empty());
            assert!(output.stdin_closed && output.stdout_eof && output.stderr_eof);
        }
        let refused = f.run(b"", OUTPUT_MAX + 1, DEADLINE);
        assert_eq!(refused.stop, Stop::OutputBudget);
        assert!(!refused.spawned);
        assert_eq!(refused.stdout.capacity(), 0);
    }
    #[test]
    fn lexical_supervisor_eof_does_not_replace_exit_or_complete_input() {
        let output = Fixture::new(9).run(&[b'x'; 131072], 0, DEADLINE);
        assert_ne!(output.stop, Stop::Exited);
        assert!(output.input_written < 131072);
        assert!(output.stdin_closed && output.status.is_some());
        let output = Fixture::new(10).run(b"", 0, Duration::from_millis(250));
        assert_eq!(output.stop, Stop::Timeout);
        assert_eq!(output.signal(), Some(libc::SIGKILL));
        assert!(output.stdout_eof && output.stderr_eof);
    }
    #[test]
    fn lexical_supervisor_descriptor_cwd_and_sealed_program_survive_renaming() {
        use sha2::{Digest, Sha256};
        let f = Fixture::new(0);
        let executable = fs::read(f.0.join("helper")).unwrap();
        fs::write(f.0.join("lexer"), &executable).unwrap();
        let hash: [u8; 32] = Sha256::digest(&executable).into();
        let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        fs::write(
            f.0.join("manifest.txt"),
            format!("oxid-lexical-provider-v1\nlexer_sha256={hex}\n"),
        )
        .unwrap();
        let bundle = super::super::bundle::Bundle::load(&f.0).unwrap();
        let cwd = PathBuf::from(format!("/proc/self/fd/{}", bundle.directory().as_raw_fd()));
        fs::write(cwd.join("mode"), b"0").unwrap();
        fs::remove_file(f.0.join("lexer")).unwrap();
        fs::write(f.0.join("lexer"), b"replaced executable").unwrap();
        let output = run(bundle.lexer(), bundle.directory(), b"sealed", 6, DEADLINE);
        assert_eq!(output.stop, Stop::Exited);
        assert_eq!(output.code(), Some(0));
        assert_eq!(&output.stdout, b"sealed");
        assert_eq!(bundle.lexer_hash(), hash);
        // Keep the directory descriptor while replacing its former pathname.
        let moved = f.0.join("renamed");
        fs::create_dir(&moved).unwrap();
        fs::write(moved.join("mode"), b"0").unwrap();
        let directory = File::open(&moved).unwrap();
        let retained = f.0.join("retained");
        fs::rename(&moved, &retained).unwrap();
        fs::create_dir(&moved).unwrap();
        fs::write(moved.join("mode"), b"3").unwrap();
        let output = run(bundle.lexer(), &directory, b"identity", 8, DEADLINE);
        assert_eq!(output.stop, Stop::Exited);
        assert_eq!(&output.stdout, b"identity");
    }
    #[test]
    fn lexical_supervisor_spawn_failure_and_zero_deadline_are_not_success() {
        let f = Fixture::new(0);
        let program = File::open(f.0.join("mode")).unwrap();
        let directory = File::open(&f.0).unwrap();
        let failed = run(&program, &directory, b"", 0, DEADLINE);
        assert_eq!(failed.stop, Stop::Spawn);
        assert!(!failed.spawned && failed.status.is_none());
        assert!(failed.stdout.is_empty());
        assert_eq!(failed.stdout.capacity(), 1);
        let output = f.run(b"", 0, Duration::ZERO);
        assert_eq!(output.stop, Stop::Timeout);
        assert!(output.status.is_some());
    }
    #[test]
    fn lexical_supervisor_reports_allocation_failure_name() {
        let mut capture = Capture::empty();
        assert!(!capture.allocate_stdout(usize::MAX));
        assert_eq!(capture.stop, Stop::Allocation);
        assert!(!capture.spawned);
        assert!(capture.stdout.is_empty());
        assert_eq!(capture.stdout.capacity(), 0);
        assert_eq!(Stop::Allocation.name(), "allocation-failed");
        assert!(named_bytes() >= std::mem::size_of::<Capture>());
    }
}
