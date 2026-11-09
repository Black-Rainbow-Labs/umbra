//! Starting child processes so Ctrl+C reaches Umbra and not them.

use std::process::Command;

/// Windows' `CREATE_NEW_PROCESS_GROUP`. A process in its own group has Ctrl+C turned off.
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Puts a child in its own process group, so a Ctrl+C typed in Umbra's terminal is not sent to
/// it. Umbra stops the child itself when it needs to. On Unix this also lets a kill reach
/// everything the child started.
/// @param process - the command being built
#[cfg(unix)]
pub fn own_process_group(process: &mut Command) {
    use std::os::unix::process::CommandExt;
    process.process_group(0);
}

/// Puts a child in its own process group, so a Ctrl+C typed in Umbra's console is not sent to
/// it. Umbra stops the child itself when it needs to, with `taskkill /T`.
/// @param process - the command being built
#[cfg(windows)]
pub fn own_process_group(process: &mut Command) {
    use std::os::windows::process::CommandExt;
    process.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

/// Other platforms have no process groups to set.
/// @param _process - the command being built
#[cfg(not(any(unix, windows)))]
pub fn own_process_group(_process: &mut Command) {}

/// Everything a command started, so all of it can be stopped. On Windows that is a job object:
/// Git Bash starts a program such as `sleep.exe` from a process outside bash's own process tree,
/// so `taskkill /T` on bash misses it, while a job holds every process started inside it.
pub struct ProcessJob {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl ProcessJob {
    /// Puts a just started child in a new job. Returns None when Windows refuses, for example
    /// when Umbra itself runs in a job that does not allow nested jobs.
    /// @param child - the child process
    pub fn track(child: &std::process::Child) -> Option<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
        // SAFETY: plain Win32 calls; the job handle is closed on failure and on drop.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }
            if AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0 {
                CloseHandle(handle);
                return None;
            }
            Some(ProcessJob { handle })
        }
    }

    /// Stops every process in the job.
    pub fn kill(&self) {
        // SAFETY: the handle is a live job handle owned by this value.
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.handle, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    /// Closes the job handle. Processes still in the job keep running, so a command that starts
    /// a server in the background is not stopped when it returns.
    fn drop(&mut self) {
        // SAFETY: the handle is a live job handle owned by this value.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
impl ProcessJob {
    /// Unix uses the process group instead, so there is nothing to track.
    /// @param _child - the child process
    pub fn track(_child: &std::process::Child) -> Option<Self> {
        None
    }

    /// Nothing to stop; the process group kill covers it.
    pub fn kill(&self) {}
}
