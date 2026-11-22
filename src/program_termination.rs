//! Opt-in termination notifications owned by one explicit process execution.
//! The OS callback records one scalar flag; ordinary source return owns cleanup.

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod platform {
    use std::{
        cell::RefCell,
        marker::PhantomData,
        mem::{size_of, MaybeUninit},
        ptr,
        rc::Rc,
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::{Duration, Instant},
    };

    // AtomicBool uses the supported ABIs' lock-free byte atomic. The handler
    // never touches thread-local state, a Rust borrow, an owner, or an allocator.
    static CLAIMED: AtomicBool = AtomicBool::new(false);
    static REQUESTED: AtomicBool = AtomicBool::new(false);

    struct Context {
        active: bool,
        enabled: bool,
        previous: [MaybeUninit<libc::sigaction>; 2],
    }
    impl Context {
        const fn new() -> Self {
            Self {
                active: false,
                enabled: false,
                previous: [MaybeUninit::uninit(), MaybeUninit::uninit()],
            }
        }
    }
    thread_local! {
        static CURRENT: RefCell<Context> = const { RefCell::new(Context::new()) };
    }

    extern "C" fn notify(_: libc::c_int) {
        REQUESTED.store(true, Ordering::Relaxed);
    }

    pub(crate) struct Scope {
        // A scope restores the context on the same thread that entered it.
        _thread: PhantomData<Rc<()>>,
    }
    impl Scope {
        pub(crate) fn enter() -> Result<Self, ()> {
            CLAIMED
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .map_err(|_| ())?;
            CURRENT.with(|current| {
                let mut context = current.borrow_mut();
                debug_assert!(!context.active && !context.enabled);
                context.active = true;
            });
            REQUESTED.store(false, Ordering::Relaxed);
            Ok(Self {
                _thread: PhantomData,
            })
        }
    }

    /// Failure to restore process policy cannot be returned as a successful
    /// rollback. This explicit process-only scope terminates silently instead.
    fn restore_or_exit(result: libc::c_int) {
        if result != 0 {
            // SAFETY: _exit has no Rust unwinding, callback or allocation path.
            unsafe { libc::_exit(74) }
        }
    }

    struct Blocked {
        previous: libc::sigset_t,
    }
    impl Blocked {
        fn begin() -> Option<Self> {
            let mut selected = MaybeUninit::<libc::sigset_t>::uninit();
            let mut previous = MaybeUninit::<libc::sigset_t>::uninit();
            // SAFETY: all outputs point to their complete C-layout objects;
            // the selected set is initialized before it is read by libc.
            unsafe {
                if libc::sigemptyset(selected.as_mut_ptr()) != 0
                    || libc::sigaddset(selected.as_mut_ptr(), libc::SIGINT) != 0
                    || libc::sigaddset(selected.as_mut_ptr(), libc::SIGTERM) != 0
                    || libc::pthread_sigmask(
                        libc::SIG_BLOCK,
                        selected.as_ptr(),
                        previous.as_mut_ptr(),
                    ) != 0
                {
                    return None;
                }
                Some(Self {
                    previous: previous.assume_init(),
                })
            }
        }
    }
    impl Drop for Blocked {
        fn drop(&mut self) {
            // SAFETY: previous is the successful pthread_sigmask output.
            restore_or_exit(unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, ptr::null_mut())
            });
        }
    }

    #[cfg(test)]
    static FAIL_INSTALL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
    #[cfg(test)]
    static INSTALL_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    #[cfg(test)]
    static DELIVERIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    #[cfg(test)]
    pub(crate) fn reset_observation() {
        INSTALL_CALLS.store(0, Ordering::Relaxed);
        DELIVERIES.store(0, Ordering::Relaxed);
    }
    #[cfg(test)]
    pub(crate) fn observation() -> (usize, usize) {
        (
            INSTALL_CALLS.load(Ordering::Relaxed),
            DELIVERIES.load(Ordering::Relaxed),
        )
    }
    #[cfg(test)]
    pub(crate) fn delivered() {
        DELIVERIES.fetch_add(1, Ordering::Relaxed);
    }

    fn install(
        signal: libc::c_int,
        action: &libc::sigaction,
        previous: *mut libc::sigaction,
        step: u8,
    ) -> bool {
        #[cfg(test)]
        INSTALL_CALLS.fetch_add(1, Ordering::Relaxed);
        #[cfg(test)]
        if FAIL_INSTALL.load(Ordering::Relaxed) == step {
            return false;
        }
        #[cfg(not(test))]
        let _ = step;
        // SAFETY: action is fully initialized and previous is writable storage
        // for one complete sigaction. Neither handler performs source work.
        unsafe { libc::sigaction(signal, action, previous) == 0 }
    }

    pub(crate) fn enable() -> bool {
        CURRENT.with(|current| {
            let Ok(mut context) = current.try_borrow_mut() else {
                return false;
            };
            if !context.active {
                return false;
            }
            if context.enabled {
                return true;
            }
            let Some(_blocked) = Blocked::begin() else {
                return false;
            };
            // SAFETY: sigaction accepts an all-zero initialized C object, then
            // its mask/handler/flags are set explicitly before installation.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = notify as *const () as usize;
            action.sa_flags = 0;
            // SAFETY: action owns a complete initialized mask.
            if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
                return false;
            }
            REQUESTED.store(false, Ordering::Relaxed);
            if !install(libc::SIGINT, &action, context.previous[0].as_mut_ptr(), 1) {
                return false;
            }
            if !install(libc::SIGTERM, &action, context.previous[1].as_mut_ptr(), 2) {
                // SAFETY: successful first installation initialized this exact
                // previous disposition; both selected signals remain blocked.
                restore_or_exit(unsafe {
                    libc::sigaction(libc::SIGINT, context.previous[0].as_ptr(), ptr::null_mut())
                });
                return false;
            }
            context.enabled = true;
            true
        })
    }

    pub(crate) fn requested() -> bool {
        CURRENT.with(|current| {
            current.try_borrow().is_ok_and(|context| {
                context.active && context.enabled && REQUESTED.load(Ordering::Relaxed)
            })
        })
    }

    pub(crate) fn wait(timeout_ms: i32) -> bool {
        let Ok(timeout_ms) = u64::try_from(timeout_ms) else {
            return false;
        };
        let enabled = CURRENT.with(|current| {
            current
                .try_borrow()
                .is_ok_and(|context| context.active && context.enabled)
        });
        if !enabled {
            return false;
        }
        let timeout = Duration::from_millis(timeout_ms);
        let started = Instant::now();
        while !REQUESTED.load(Ordering::Relaxed) {
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return REQUESTED.load(Ordering::Relaxed);
            }
            // A notification may be delivered to another thread between the
            // poll and the sleep. Bound the delay before the next poll.
            thread::sleep((timeout - elapsed).min(Duration::from_millis(10)));
        }
        true
    }

    #[allow(dead_code)]
    struct WaitControl {
        start: Instant,
        timeout: Duration,
        elapsed: Duration,
        remaining: Duration,
        delay: Duration,
        timeout_ms: u64,
        enabled: bool,
        result: bool,
    }
    pub(crate) const fn wait_bytes() -> usize {
        size_of::<WaitControl>()
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            CURRENT.with(|current| {
                let mut context = current.borrow_mut();
                if context.enabled {
                    let Some(_blocked) = Blocked::begin() else {
                        // SAFETY: failed restoration never returns into an
                        // embedding host with altered process dispositions.
                        unsafe { libc::_exit(74) }
                    };
                    // SAFETY: enabled proves both previous objects were
                    // initialized by successful installation in this scope.
                    restore_or_exit(unsafe {
                        libc::sigaction(libc::SIGINT, context.previous[0].as_ptr(), ptr::null_mut())
                    });
                    restore_or_exit(unsafe {
                        libc::sigaction(
                            libc::SIGTERM,
                            context.previous[1].as_ptr(),
                            ptr::null_mut(),
                        )
                    });
                    context.enabled = false;
                }
                context.active = false;
            });
            REQUESTED.store(false, Ordering::Relaxed);
            CLAIMED.store(false, Ordering::Release);
        }
    }

    // Independent fixed roles for the new family only, separate from the
    // existing execution carriers and scalar/vector admission formulas.
    #[allow(dead_code)]
    struct Control {
        scope: Scope,
        scope_slot: Option<Scope>,
        scope_return: Result<Scope, ()>,
        installed_context: RefCell<Context>,
        context_borrow: std::cell::RefMut<'static, Context>,
        context_read: std::cell::Ref<'static, Context>,
        read_return: Result<std::cell::Ref<'static, Context>, std::cell::BorrowError>,
        write_return: Result<std::cell::RefMut<'static, Context>, std::cell::BorrowMutError>,
        block: Blocked,
        block_return: Option<Blocked>,
        selected_mask: MaybeUninit<libc::sigset_t>,
        previous_mask: MaybeUninit<libc::sigset_t>,
        action: libc::sigaction,
        claim_return: Result<bool, bool>,
        booleans: [bool; 8],
        statuses: [libc::c_int; 8],
        signal: libc::c_int,
        action_argument: &'static libc::sigaction,
        previous_argument: *mut libc::sigaction,
        step: u8,
    }
    pub(crate) const fn control_bytes() -> usize {
        size_of::<Control>()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::process::Command;
        static PREVIOUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
        extern "C" fn previous(number: libc::c_int) {
            PREVIOUS.store(number, Ordering::Relaxed);
        }
        fn action(number: libc::c_int) -> libc::sigaction {
            let mut value = MaybeUninit::uninit();
            assert_eq!(
                unsafe { libc::sigaction(number, ptr::null(), value.as_mut_ptr()) },
                0
            );
            unsafe { value.assume_init() }
        }
        fn same(number: libc::c_int, expected: &libc::sigaction) {
            let actual = action(number);
            assert_eq!(actual.sa_sigaction, expected.sa_sigaction);
            assert_eq!(actual.sa_flags, expected.sa_flags);
            for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGUSR1] {
                assert_eq!(
                    unsafe { libc::sigismember(&actual.sa_mask, signal) },
                    unsafe { libc::sigismember(&expected.sa_mask, signal) }
                );
            }
        }
        #[test]
        fn child() {
            let Ok(mode) = std::env::var("OXID_TERMINATION_HELPER_CHILD") else {
                return;
            };
            let mut original: libc::sigaction = unsafe { std::mem::zeroed() };
            original.sa_sigaction = previous as *const () as usize;
            original.sa_flags = libc::SA_RESTART;
            assert_eq!(unsafe { libc::sigemptyset(&mut original.sa_mask) }, 0);
            assert_eq!(
                unsafe { libc::sigaddset(&mut original.sa_mask, libc::SIGUSR1) },
                0
            );
            for signal in [libc::SIGINT, libc::SIGTERM] {
                assert_eq!(
                    unsafe { libc::sigaction(signal, &original, ptr::null_mut()) },
                    0
                );
            }
            let before_int = action(libc::SIGINT);
            let before_term = action(libc::SIGTERM);
            assert!(!enable());
            assert!(!requested());
            {
                let scope = Scope::enter().unwrap();
                assert!(Scope::enter().is_err());
                assert_eq!(unsafe { libc::raise(libc::SIGINT) }, 0);
                assert_eq!(PREVIOUS.load(Ordering::Relaxed), libc::SIGINT);
                assert!(!requested());
                match mode.as_str() {
                    "first-install-fails" | "second-install-fails" => {
                        FAIL_INSTALL.store(
                            if mode == "first-install-fails" { 1 } else { 2 },
                            Ordering::Relaxed,
                        );
                        assert!(!enable());
                        assert!(!requested());
                        same(libc::SIGINT, &before_int);
                        same(libc::SIGTERM, &before_term);
                        FAIL_INSTALL.store(0, Ordering::Relaxed);
                    }
                    "sticky" => {
                        assert!(enable());
                        assert!(!requested());
                        assert_eq!(unsafe { libc::raise(libc::SIGINT) }, 0);
                        assert!(requested());
                        assert!(enable());
                        assert!(requested());
                        assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
                        assert!(requested());
                    }
                    "timed-wait" => {
                        assert!(!wait(i32::MAX));
                        assert!(enable());
                        assert!(!wait(0));
                        let started = Instant::now();
                        assert!(!wait(25));
                        assert!(started.elapsed() >= Duration::from_millis(25));
                        assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
                        assert!(!wait(-1));
                        assert!(wait(0));
                        assert!(wait(i32::MAX));
                    }
                    "early-return" => {
                        assert!(enable());
                        drop(scope);
                        same(libc::SIGINT, &before_int);
                        same(libc::SIGTERM, &before_term);
                        assert!(!requested());
                        return;
                    }
                    "error-return" => {
                        fn fail() -> Result<(), ()> {
                            let _scope = Scope::enter()?;
                            assert!(enable());
                            Err(())
                        }
                        drop(scope);
                        assert_eq!(fail(), Err(()));
                        same(libc::SIGINT, &before_int);
                        same(libc::SIGTERM, &before_term);
                        return;
                    }
                    _ => panic!("unknown child mode"),
                }
            }
            same(libc::SIGINT, &before_int);
            same(libc::SIGTERM, &before_term);
            assert!(!enable());
            assert!(!requested());
            assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
            assert_eq!(PREVIOUS.load(Ordering::Relaxed), libc::SIGTERM);
            let _second = Scope::enter().unwrap();
            assert!(enable());
            assert!(!requested());
        }
        #[test]
        fn notifications_and_restoration_run_only_in_isolated_children() {
            for mode in [
                "first-install-fails",
                "second-install-fails",
                "sticky",
                "timed-wait",
                "early-return",
                "error-return",
            ] {
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "program_termination::platform::tests::child",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env("OXID_TERMINATION_HELPER_CHILD", mode)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{mode}: {output:?}");
            }
        }
    }
}

#[cfg(not(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
mod platform {
    pub(crate) struct Scope;
    impl Scope {
        pub(crate) fn enter() -> Result<Self, ()> {
            Err(())
        }
    }
    pub(crate) fn enable() -> bool {
        false
    }
    pub(crate) fn requested() -> bool {
        false
    }
    pub(crate) fn wait(_: i32) -> bool {
        false
    }
    pub(crate) const fn wait_bytes() -> usize {
        0
    }
    pub(crate) const fn control_bytes() -> usize {
        0
    }
    #[cfg(test)]
    pub(crate) fn reset_observation() {}
    #[cfg(test)]
    pub(crate) fn observation() -> (usize, usize) {
        (0, 0)
    }
    #[cfg(test)]
    pub(crate) fn delivered() {}
}

pub(crate) use platform::{control_bytes, enable, requested, wait, wait_bytes, Scope};
#[cfg(test)]
pub(crate) use platform::{delivered, observation, reset_observation};

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(crate) fn monotonic_millis() -> i64 {
    let mut sample = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: sample points to a full writable timespec. A successful clock
    // read initializes both fields before they are read below.
    let sample = unsafe {
        if libc::clock_gettime(libc::CLOCK_MONOTONIC, sample.as_mut_ptr()) != 0 {
            return -1;
        }
        sample.assume_init()
    };
    if sample.tv_sec < 0 || !(0..1_000_000_000).contains(&sample.tv_nsec) {
        return -1;
    }
    sample
        .tv_sec
        .checked_mul(1000)
        .and_then(|seconds| seconds.checked_add(sample.tv_nsec / 1_000_000))
        .unwrap_or(-1)
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub(crate) fn monotonic_millis() -> i64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount64() -> u64;
    }
    // SAFETY: GetTickCount64 takes no arguments and returns a scalar.
    i64::try_from(unsafe { GetTickCount64() }).unwrap_or(-1)
}

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub(crate) fn monotonic_millis() -> i64 {
    -1
}

#[allow(dead_code)]
struct ClockCarriers {
    clock_sample: [i64; 2],
    clock_status: i32,
    millis: i64,
    checked_millis: Option<i64>,
    returned: i64,
}

pub(crate) const fn clock_bytes() -> usize {
    std::mem::size_of::<ClockCarriers>()
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(crate) fn unix_millis() -> i64 {
    let mut sample = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: a successful realtime read initializes the writable timespec.
    let sample = unsafe {
        if libc::clock_gettime(libc::CLOCK_REALTIME, sample.as_mut_ptr()) != 0 {
            return i64::MIN;
        }
        sample.assume_init()
    };
    epoch_millis(sample.tv_sec, sample.tv_nsec)
}

#[cfg(any(
    test,
    all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn epoch_millis(seconds: i64, nanos: i64) -> i64 {
    if !(0..1_000_000_000).contains(&nanos) {
        return i64::MIN;
    }
    let millis = i128::from(seconds) * 1000 + i128::from(nanos / 1_000_000);
    i64::try_from(millis).unwrap_or(i64::MIN)
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub(crate) fn unix_millis() -> i64 {
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemTimeAsFileTime(time: *mut FileTime);
    }
    let mut time = std::mem::MaybeUninit::<FileTime>::uninit();
    // SAFETY: the Windows API writes both 32-bit words of the FILETIME.
    let time = unsafe {
        GetSystemTimeAsFileTime(time.as_mut_ptr());
        time.assume_init()
    };
    let ticks = (u64::from(time.high) << 32) | u64::from(time.low);
    (ticks / 10_000) as i64 - 11_644_473_600_000
}

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
pub(crate) fn unix_millis() -> i64 {
    i64::MIN
}

#[allow(dead_code)]
struct UnixClockCarriers {
    sample: [i64; 2],
    sample_pointer: *mut [i64; 2],
    file_time: [u32; 2],
    status: i32,
    realtime_ticks: u64,
    ms_since_epoch: i128,
    signed_conversion: Result<i64, std::num::TryFromIntError>,
    returned: i64,
}

pub(crate) const fn unix_clock_bytes() -> usize {
    std::mem::size_of::<UnixClockCarriers>()
}

#[cfg(test)]
#[test]
fn unix_time_rounds_toward_the_start_of_a_millisecond() {
    for (seconds, nanos, expected) in [
        (-1, 999_999_999, -1),
        (0, 999_999, 0),
        (0, 1_000_000, 1),
        (-9_223_372_036_854_776, 193_000_000, i64::MIN + 1),
        (9_223_372_036_854_775, 807_999_999, i64::MAX),
        (9_223_372_036_854_775, 808_000_000, i64::MIN),
        (-9_223_372_036_854_776, 191_999_999, i64::MIN),
        (1, -1, i64::MIN),
        (1, 1_000_000_000, i64::MIN),
    ] {
        assert_eq!(epoch_millis(seconds, nanos), expected);
    }
}

pub(crate) fn sleep_millis(milliseconds: i32) -> bool {
    if milliseconds <= 0 {
        return milliseconds == 0;
    }
    sleep_positive(milliseconds as u32)
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn sleep_positive(milliseconds: u32) -> bool {
    let mut request = libc::timespec {
        tv_sec: i64::from(milliseconds / 1000),
        tv_nsec: i64::from(milliseconds % 1000) * 1_000_000,
    };
    let mut remaining = std::mem::MaybeUninit::<libc::timespec>::uninit();
    loop {
        // SAFETY: request and remaining are distinct C-layout timespecs.
        if unsafe { libc::nanosleep(&request, remaining.as_mut_ptr()) } == 0 {
            return true;
        }
        if std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return false;
        }
        // SAFETY: nanosleep writes the unslept interval on EINTR.
        request = unsafe { remaining.assume_init() };
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn sleep_positive(milliseconds: u32) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn Sleep(milliseconds: u32);
    }
    // SAFETY: the positive i32 duration fits DWORD and is below INFINITE.
    unsafe { Sleep(milliseconds) };
    true
}

#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
fn sleep_positive(_: u32) -> bool {
    false
}

#[allow(dead_code)]
struct SleepCarriers {
    request: [i64; 2],
    remaining: [i64; 2],
    duration: u32,
    status: i32,
    os_error: std::io::Error,
    completed: bool,
}

pub(crate) const fn sleep_bytes() -> usize {
    std::mem::size_of::<SleepCarriers>()
}
