//! Keeping the shell's footprint where it was before a panel opened.

/// Give freed heap memory back to the system. A panel draws many large frames
/// while it moves; glibc keeps that memory after it is freed unless asked, and
/// the shell then stayed about 7 MiB above idle after closing (test VM).
pub fn give_back() {
    #[cfg(target_env = "gnu")]
    // SAFETY: malloc_trim only touches glibc's own allocator state.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// Make a child process end with the shell, however the shell ends. Ending it
/// on a clean exit alone missed crashes and lost compositor connections: the
/// test VM collected stray `pactl subscribe` processes that way.
pub fn tie_to_shell(launcher: &gtk::gio::SubprocessLauncher) {
    // Runs in the child between fork and exec; prctl is safe to call there.
    launcher.set_child_setup(|| {
        // SAFETY: a plain syscall with constant arguments.
        unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
        }
    });
}
