//! Linux bounded-stdio supervisor for explicitly selected local producers.
//! This is process lifetime/transport control, not an OS sandbox or RSS bound.
use std::{
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) const INPUT_MAX: usize = 1692;
pub(super) const OUTPUT_MAX: usize = 2607;
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
    InputLimit,
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
            Self::InputLimit => "input-limit",
        }
    }
}

pub(super) struct Capture {
    pub(super) stdout: [u8; OUTPUT_MAX + 1],
    pub(super) stdout_len: usize,
    pub(super) stderr: [u8; STDERR_MAX + 1],
    pub(super) stderr_len: usize,
    pub(super) input_written: usize,
    pub(super) status: Option<ExitStatus>,
    pub(super) stop: Stop,
    pub(super) spawned: bool,
}
impl Capture {
    fn empty() -> Self {
        Self {
            stdout: [0; OUTPUT_MAX + 1],
            stdout_len: 0,
            stderr: [0; STDERR_MAX + 1],
            stderr_len: 0,
            input_written: 0,
            status: None,
            stop: Stop::Spawn,
            spawned: false,
        }
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

fn drain(reader: &mut impl Read, bytes: &mut [u8], used: &mut usize) -> io::Result<bool> {
    while *used < bytes.len() {
        match reader.read(&mut bytes[*used..]) {
            Ok(0) => return Ok(true),
            Ok(count) => *used += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

pub(super) fn run(
    program: &Path,
    directory: &Path,
    input: &[u8],
    output_limit: usize,
    deadline: Duration,
) -> Capture {
    let mut captured = Capture::empty();
    if input.len() > INPUT_MAX || output_limit > OUTPUT_MAX {
        captured.stop = Stop::InputLimit;
        return captured;
    }
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .current_dir(directory)
        .env_clear()
        .env("PATH", "/no-tools")
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: pre_exec uses only async-signal-safe Linux syscalls and constructs
    // an errno value on failure. No allocation, locks or inherited Rust state.
    unsafe {
        command.pre_exec(move || {
            // A fresh session prevents the producer leader from joining the
            // compiler group. Deliberately escaping descendants are outside
            // this trusted-producer lifetime control, not silently sandboxed.
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
        captured.status = group.finish();
        return captured;
    }
    let mut out_eof = false;
    let mut err_eof = false;
    loop {
        if started.elapsed() >= deadline {
            captured.stop = Stop::Timeout;
            break;
        }
        if captured.input_written == input.len() {
            stdin.take();
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
        if !out_eof {
            match drain(
                &mut stdout,
                &mut captured.stdout[..output_limit + 1],
                &mut captured.stdout_len,
            ) {
                Ok(eof) => out_eof = eof,
                Err(_) => break,
            }
        }
        if captured.stdout_len > output_limit {
            captured.stop = Stop::StdoutLimit;
            break;
        }
        if !err_eof {
            match drain(&mut stderr, &mut captured.stderr, &mut captured.stderr_len) {
                Ok(eof) => err_eof = eof,
                Err(_) => break,
            }
        }
        if captured.stderr_len > STDERR_MAX {
            captured.stop = Stop::StderrLimit;
            break;
        }
        match exited(&group.child) {
            Ok(true) if out_eof && err_eof && captured.input_written == input.len() => {
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
    captured.status = group.finish();
    if captured.stop == Stop::Exited {
        // Recheck the fixed pipes after termination/reap. A prior EOF is not
        // treated as authority for a process that has not yet exited.
        let final_out = drain(
            &mut stdout,
            &mut captured.stdout[..output_limit + 1],
            &mut captured.stdout_len,
        );
        let final_err = drain(&mut stderr, &mut captured.stderr, &mut captured.stderr_len);
        if captured.stdout_len > output_limit {
            captured.stop = Stop::StdoutLimit;
        } else if captured.stderr_len > STDERR_MAX {
            captured.stop = Stop::StderrLimit;
        } else if !matches!((final_out, final_err), (Ok(true), Ok(true)))
            || captured.status.is_none()
        {
            captured.stop = Stop::Io;
        }
    }
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
                "oxid-producer-supervisor-{}-{}",
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
            run(&self.0.join("helper"), &self.0, input, cap, deadline)
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
    fn producer_supervisor_fragmented_stdio_is_exact_and_leader_reaped() {
        let f = Fixture::new(0);
        let output = f.run(b"hello", 5, DEADLINE);
        assert_eq!(output.stop, Stop::Exited);
        assert_eq!(output.code(), Some(0));
        assert_eq!(&output.stdout[..output.stdout_len], b"hello");
        assert_eq!(output.stderr_len, 0);
        assert_eq!(output.input_written, 5);
    }
    #[test]
    fn producer_supervisor_stream_caps_use_one_witness_and_fail_closed() {
        let output = Fixture::new(1).run(b"", 8, DEADLINE);
        assert_eq!(output.stop, Stop::StdoutLimit);
        assert_eq!(output.stdout_len, 9);
        assert!(output.status.is_some());
        let output = Fixture::new(2).run(b"", 8, DEADLINE);
        assert_eq!(output.stop, Stop::StderrLimit);
        assert_eq!(output.stderr_len, STDERR_MAX + 1);
        assert!(output.status.is_some());
    }
    #[test]
    fn producer_supervisor_timeout_cannot_be_defeated_by_joining_parent_group() {
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
    fn producer_supervisor_kills_group_descendants_after_success_and_timeout() {
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
    fn producer_supervisor_executes_sealed_descriptors_across_peer_path_replacement() {
        use sha2::{Digest, Sha256};
        let f = Fixture::new(0);
        let executable = fs::read(f.0.join("helper")).unwrap();
        fs::write(f.0.join("parser"), &executable).unwrap();
        fs::write(f.0.join("static"), &executable).unwrap();
        let hash: [u8; 32] = Sha256::digest(&executable).into();
        let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        fs::write(
            f.0.join("manifest.txt"),
            format!("OXID-HIR-PRODUCERS-1\nparser {hex}\nstatic {hex}\n"),
        )
        .unwrap();
        let bundle = super::super::bundle::Bundle::load(&f.0).unwrap();
        fs::write(bundle.directory().join("mode"), b"7").unwrap();
        let parser = run(bundle.parser(), bundle.directory(), b"parser", 6, DEADLINE);
        assert_eq!(parser.stop, Stop::Exited);
        assert_eq!(parser.code(), Some(0));
        assert_eq!(&parser.stdout[..parser.stdout_len], b"parser");
        assert_eq!(
            fs::read(bundle.directory().join("static")).unwrap(),
            b"replaced cwd executable"
        );
        fs::write(f.0.join("static"), b"replaced source executable").unwrap();
        fs::write(bundle.directory().join("mode"), b"0").unwrap();
        let typed = run(
            bundle.static_consumer(),
            bundle.directory(),
            b"static",
            6,
            DEADLINE,
        );
        assert_eq!(typed.stop, Stop::Exited);
        assert_eq!(typed.code(), Some(0));
        assert_eq!(&typed.stdout[..typed.stdout_len], b"static");
        assert_eq!(bundle.static_hash(), hash);
    }

    #[test]
    fn producer_supervisor_input_limit_and_spawn_failure_are_not_success() {
        let f = Fixture::new(0);
        let output = f.run(&[0; INPUT_MAX + 1], 0, DEADLINE);
        assert_eq!(output.stop, Stop::InputLimit);
        assert!(!output.spawned);
        let output = run(&f.0.join("absent"), &f.0, b"", 0, DEADLINE);
        assert_eq!(output.stop, Stop::Spawn);
        assert!(!output.spawned);
        assert!(output.status.is_none());
    }
}
