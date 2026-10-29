//! A Windows Job Object that holds one translator run's whole process tree (plan T17b).
//!
//! The run is spawned suspended, put into the job and only then resumed, so every process it
//! starts is in the job from the first instruction. The job has
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: terminating it or closing its last handle (also when the
//! app itself dies) kills every process in it.

use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::os::windows::io::RawHandle;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// Exit code given to the processes a terminated job kills.
const KILLED_EXIT_CODE: u32 = 1;

/// An owned handle, closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and owned by this value; it is closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}

/// An unnamed job with kill-on-close. Dropping it kills whatever is still in it.
pub struct Job(Handle);

// SAFETY: a job handle is a kernel object handle; the job functions used here are safe to call
// from any thread.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    pub fn new() -> io::Result<Self> {
        // SAFETY: both pointers may be null (default security, unnamed job).
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self(Handle(handle));
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `info` is a valid JOBOBJECT_EXTENDED_LIMIT_INFORMATION of the given size.
        let ok = unsafe {
            SetInformationJobObject(
                job.0.0,
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&info).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Puts a process (by its handle) into the job. Nested jobs work since Windows 8, so this
    /// also works when the app itself runs inside a job (e.g. started from a terminal).
    pub fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: both handles are valid for the duration of the call.
        let ok = unsafe { AssignProcessToJobObject(self.0.0, process as HANDLE) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Kills every process in the job now.
    pub fn terminate(&self) {
        // SAFETY: the job handle is valid.
        if unsafe { TerminateJobObject(self.0.0, KILLED_EXIT_CODE) } == 0 {
            log::warn!(
                "Terminating the translator job failed: {}",
                io::Error::last_os_error()
            );
        }
    }
}

/// Resumes the threads of a process spawned with `CREATE_SUSPENDED` (it has only its main
/// thread). Fails if no thread of `pid` could be resumed.
pub fn resume_process(pid: u32) -> io::Result<()> {
    // SAFETY: a thread snapshot of the whole system; the pid argument is ignored for threads.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = Handle(snapshot);
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut resumed = 0;
    let mut last_error = None;
    // SAFETY: `entry` is a THREADENTRY32 with `dwSize` set, as the ToolHelp functions require.
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            match resume_thread(entry.th32ThreadID) {
                Ok(()) => resumed += 1,
                Err(e) => last_error = Some(e),
            }
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    if resumed > 0 {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| io::Error::other("the process has no threads")))
    }
}

fn resume_thread(tid: u32) -> io::Result<()> {
    // SAFETY: opening a thread by id; a null result is handled.
    let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) };
    if thread.is_null() {
        return Err(io::Error::last_os_error());
    }
    let thread = Handle(thread);
    // SAFETY: the handle has THREAD_SUSPEND_RESUME access.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
