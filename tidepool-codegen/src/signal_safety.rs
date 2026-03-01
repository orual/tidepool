//! JIT signal safety via sigsetjmp/siglongjmp.
//!
//! JIT-compiled code can crash with SIGILL (case trap `ud2`) or SIGSEGV
//! (bad memory access). This module provides `with_signal_protection` which
//! wraps JIT calls so that these signals return a clean error instead of
//! killing the process.
//!
//! This is the standard technique used by real JIT runtimes (Wasmtime, V8,
//! SpiderMonkey) to recover from signals in generated code.

#[cfg(unix)]
mod inner {
    use std::ptr::{self, null_mut};
    use std::sync::atomic::{AtomicPtr, Ordering};

    // sigjmp_buf sizes vary by platform:
    //   - Linux x86_64 (glibc): __jmp_buf_tag[1] = 200 bytes
    //   - macOS x86_64: 37 ints + signal mask ≈ 296 bytes
    //   - macOS aarch64: similar, ~304 bytes
    // Use 512 bytes to cover all platforms with headroom.
    #[repr(C, align(16))]
    pub struct SigJmpBuf {
        _buf: [u8; 512],
    }

    extern "C" {
        // On Linux (glibc), sigsetjmp is a macro that calls __sigsetjmp.
        // On macOS, sigsetjmp is a real function.
        #[cfg(target_os = "linux")]
        fn __sigsetjmp(env: *mut SigJmpBuf, savesigs: libc::c_int) -> libc::c_int;
        #[cfg(not(target_os = "linux"))]
        fn sigsetjmp(env: *mut SigJmpBuf, savesigs: libc::c_int) -> libc::c_int;

        fn siglongjmp(env: *mut SigJmpBuf, val: libc::c_int) -> !;
    }

    #[cfg(target_os = "linux")]
    unsafe fn platform_sigsetjmp(env: *mut SigJmpBuf, savesigs: libc::c_int) -> libc::c_int {
        unsafe { __sigsetjmp(env, savesigs) }
    }

    #[cfg(not(target_os = "linux"))]
    unsafe fn platform_sigsetjmp(env: *mut SigJmpBuf, savesigs: libc::c_int) -> libc::c_int {
        unsafe { sigsetjmp(env, savesigs) }
    }

    /// Signal number that caused the jump.
    #[derive(Debug, Clone, Copy)]
    pub struct SignalError(pub i32);

    impl std::fmt::Display for SignalError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let name = match self.0 {
                libc::SIGILL => "SIGILL (illegal instruction — likely exhausted case branch)",
                libc::SIGSEGV => "SIGSEGV (segmentation fault — likely invalid memory access)",
                libc::SIGBUS => "SIGBUS (bus error)",
                _ => "unknown signal",
            };
            write!(f, "JIT signal: {}", name)
        }
    }

    /// Global jump buffer pointer. Only one JIT execution at a time
    /// (MCP server is single-eval).
    static JMP_BUF: AtomicPtr<SigJmpBuf> = AtomicPtr::new(ptr::null_mut());

    /// Wrap a JIT call with signal protection.
    ///
    /// If SIGILL/SIGSEGV/SIGBUS fires during `f()`, returns `Err(SignalError)`
    /// instead of crashing the process.
    ///
    /// # Safety
    ///
    /// The closure `f` must not hold Rust objects with Drop impls that would be
    /// skipped by siglongjmp. Raw pointers and references are fine.
    pub unsafe fn with_signal_protection<F, R>(f: F) -> Result<R, SignalError>
    where
        F: FnOnce() -> R,
    {
        let mut buf: SigJmpBuf = std::mem::zeroed();
        let val = platform_sigsetjmp(&mut buf, 1); // savesigs=1
        if val != 0 {
            // Jumped back from signal handler
            JMP_BUF.store(null_mut(), Ordering::Relaxed);
            return Err(SignalError(val));
        }
        JMP_BUF.store(&mut buf, Ordering::Relaxed);
        let result = f();
        JMP_BUF.store(null_mut(), Ordering::Relaxed);
        Ok(result)
    }

    extern "C" fn handler(
        sig: libc::c_int,
        _info: *mut libc::siginfo_t,
        _ctx: *mut libc::c_void,
    ) {
        let buf = JMP_BUF.load(Ordering::Relaxed);
        if !buf.is_null() {
            // In JIT context — jump back to sigsetjmp
            unsafe {
                siglongjmp(buf, sig);
            }
        }
        // Not in JIT context — restore default handler and re-raise
        unsafe {
            libc::signal(sig, libc::SIG_DFL);
            libc::raise(sig);
        }
    }

    /// Install signal handlers for SIGILL, SIGSEGV, SIGBUS on an alternate stack.
    ///
    /// Idempotent — safe to call multiple times. Uses `sigaltstack` so the handler
    /// works even on stack overflow.
    pub fn install() {
        use std::alloc::{alloc, Layout};
        use std::sync::atomic::AtomicBool;

        static INSTALLED: AtomicBool = AtomicBool::new(false);
        if INSTALLED.swap(true, Ordering::SeqCst) {
            return;
        }

        const ALT_STACK_SIZE: usize = 64 * 1024;

        unsafe {
            // Allocate alternate signal stack
            let layout = Layout::from_size_align(ALT_STACK_SIZE, 16).unwrap();
            let alt_stack_ptr = alloc(layout);
            if alt_stack_ptr.is_null() {
                return;
            }

            let stack = libc::stack_t {
                ss_sp: alt_stack_ptr as *mut libc::c_void,
                ss_flags: 0,
                ss_size: ALT_STACK_SIZE,
            };
            libc::sigaltstack(&stack, ptr::null_mut());

            // Install handler for SIGILL, SIGSEGV, SIGBUS
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
            sa.sa_sigaction = handler as *const () as usize;
            libc::sigemptyset(&mut sa.sa_mask);

            libc::sigaction(libc::SIGILL, &sa, ptr::null_mut());
            libc::sigaction(libc::SIGSEGV, &sa, ptr::null_mut());
            libc::sigaction(libc::SIGBUS, &sa, ptr::null_mut());
        }
    }
}

#[cfg(not(unix))]
mod inner {
    #[derive(Debug, Clone, Copy)]
    pub struct SignalError(pub i32);

    impl std::fmt::Display for SignalError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "JIT signal: {}", self.0)
        }
    }

    pub unsafe fn with_signal_protection<F, R>(f: F) -> Result<R, SignalError>
    where
        F: FnOnce() -> R,
    {
        Ok(f())
    }

    pub fn install() {}
}

pub use inner::*;
