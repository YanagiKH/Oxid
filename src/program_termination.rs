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

pub(crate) use platform::{control_bytes, enable, requested, Scope};
#[cfg(test)]
pub(crate) use platform::{delivered, observation, reset_observation};
