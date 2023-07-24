//! Utilities for keeping track of jobs, processes and subshells, as well as signal handling
//! functions for tracking children. These functions do not themselves launch new processes,
//! the exec library will call proc to create representations of the running jobs as needed.

use crate::{
    common::redirect_tty_output,
    flog::{FLOG, FLOGF},
    job_group::JobGroup,
    wutil::{perror, wgettext},
};
use libc::{self, EBADF, EINVAL, ENOTTY, EPERM, STDIN_FILENO, WNOHANG};
use std::sync::{Arc, RwLock};

// Port note: it might be possible to simplify this to just Arc<JobGroup>, but
// the tmodes would need to made atomic too
pub type JobGroupRef = Arc<RwLock<JobGroup>>;

// Allows transferring the tty to a job group, while it runs.
#[derive(Default)]
pub struct TtyTransfer<'a> {
    // The job group which owns the tty, or empty if none.
    //    owner: Option<JobGroupRef>,
    owner: Option<&'a JobGroup>,
}

impl<'a> TtyTransfer<'a> {
    pub fn new() -> Self {
        Default::default()
    }
    /// Transfer to the given job group, if it wants to own the terminal.
    //    #[allow(clippy::wrong_self_convention)]
    //    pub fn to_job_group(&mut self, jg: &JobGroupRef) {
    pub fn to_job_group(&'a mut self, jg: &'a JobGroup) {
        assert!(self.owner.is_some(), "Terminal already transferred");
        //        if TtyTransfer::try_transfer(&jg.read().unwrap()) {
        if TtyTransfer::try_transfer(jg) {
            //            self.owner = Some(jg.clone());
            self.owner = Some(jg);
        }
    }

    /// Reclaim the tty if we transferred it.
    pub fn reclaim(&mut self) {
        if self.owner.is_some() {
            FLOG!(proc_pgroup, "fish reclaiming terminal");
            if unsafe { libc::tcsetpgrp(STDIN_FILENO, libc::getpgrp()) } == -1 {
                FLOGF!(warning, wgettext!("Could not return shell to foreground"));
                perror("tcsetpgrp");
            }
        }
        self.owner = None;
    }

    /// Save the current tty modes into the owning job group, if we are transferred.
    pub fn save_tty_modes(&mut self) {
        if let Some(ref mut owner) = self.owner {
            let mut tmodes: libc::termios = unsafe { std::mem::zeroed() };
            if unsafe { libc::tcgetattr(STDIN_FILENO, &mut tmodes) } == 0 {
                //                owner.write().unwrap().tmodes = Some(tmodes);
                owner.tmodes = Some(tmodes);
            } else if errno::errno().0 != ENOTTY {
                perror("tcgetattr");
            }
        }
    }

    fn try_transfer(jg: &JobGroup) -> bool {
        if !jg.wants_terminal() {
            // The job doesn't want the terminal.
            return false;
        }

        // Get the pgid; we must have one if we want the terminal.
        let pgid = jg.get_pgid().unwrap();
        assert!(pgid >= 0, "Invalid pgid");

        // It should never be fish's pgroup.
        let fish_pgrp = unsafe { libc::getpgrp() };
        assert!(pgid != fish_pgrp, "Job should not have fish's pgroup");

        // Ok, we want to transfer to the child.
        // Note it is important to be very careful about calling tcsetpgrp()!
        // fish ignores SIGTTOU which means that it has the power to reassign the tty even if it doesn't
        // own it. This means that other processes may get SIGTTOU and become zombies.
        // Check who own the tty now. There's four cases of interest:
        //   1. There is no tty at all (tcgetpgrp() returns -1). For example running from a pure script.
        //      Of course do not transfer it in that case.
        //   2. The tty is owned by the process. This comes about often, as the process will call
        //      tcsetpgrp() on itself between fork ane exec. This is the essential race inherent in
        //      tcsetpgrp(). In this case we want to reclaim the tty, but do not need to transfer it
        //      ourselves since the child won the race.
        //   3. The tty is owned by a different process. This may come about if fish is running in the
        //      background with job control enabled. Do not transfer it.
        //   4. The tty is owned by fish. In that case we want to transfer the pgid.
        let current_owner = unsafe { libc::tcgetpgrp(STDIN_FILENO) };
        if current_owner < 0 {
            // Case 1.
            return false;
        } else if current_owner == pgid {
            // Case 2.
            return true;
        } else if current_owner != pgid && current_owner != fish_pgrp {
            // Case 3.
            return false;
        }
        // Case 4 - we do want to transfer it.

        // The tcsetpgrp(2) man page says that EPERM is thrown if "pgrp has a supported value, but
        // is not the process group ID of a process in the same session as the calling process."
        // Since we _guarantee_ that this isn't the case (the child calls setpgid before it calls
        // SIGSTOP, and the child was created in the same session as us), it seems that EPERM is
        // being thrown because of an caching issue - the call to tcsetpgrp isn't seeing the
        // newly-created process group just yet. On this developer's test machine (WSL running Linux
        // 4.4.0), EPERM does indeed disappear on retry. The important thing is that we can
        // guarantee the process isn't going to exit while we wait (which would cause us to possibly
        // block indefinitely).
        while unsafe { libc::tcsetpgrp(STDIN_FILENO, pgid) } != 0 {
            FLOGF!(proc_termowner, "tcsetpgrp failed: %d", errno::errno());

            // Before anything else, make sure that it's even necessary to call tcsetpgrp.
            // Since it usually _is_ necessary, we only check in case it fails so as to avoid the
            // unnecessary syscall and associated context switch, which profiling has shown to have
            // a significant cost when running process groups in quick succession.
            let getpgrp_res = unsafe { libc::tcgetpgrp(STDIN_FILENO) };
            if getpgrp_res < 0 {
                match errno::errno().0 {
                    ENOTTY => {
                        // stdin is not a tty. This may come about if job control is enabled but we are
                        // not a tty - see #6573.
                        return false;
                    }
                    EBADF => {
                        // stdin has been closed. Workaround a glibc bug - see #3644.
                        redirect_tty_output();
                        return false;
                    }
                    _ => {
                        perror("tcgetpgrp");
                        return false;
                    }
                }
            }
            if getpgrp_res == pgid {
                FLOGF!(
                    proc_termowner,
                    "Process group %d already has control of terminal",
                    pgid
                );
                return true;
            }

            // Port note: this variable was set to false in C++, but all flows that end up reading it
            // also have set it to true.
            let pgroup_terminated;
            if errno::errno().0 == EINVAL {
                // OS X returns EINVAL if the process group no longer lives. Probably other OSes,
                // too. Unlike EPERM below, EINVAL can only happen if the process group has
                // terminated.
                pgroup_terminated = true;
            } else if errno::errno().0 == EPERM {
                // Retry so long as this isn't because the process group is dead.
                let wait_result = unsafe { libc::waitpid(-pgid, std::ptr::null_mut(), WNOHANG) };
                if wait_result == -1 {
                    // Note that -1 is technically an "error" for waitpid in the sense that an
                    // invalid argument was specified because no such process group exists any
                    // longer. This is the observed behavior on Linux 4.4.0. a "success" result
                    // would mean processes from the group still exist but is still running in some
                    // state or the other.
                    pgroup_terminated = true;
                } else {
                    // Debug the original tcsetpgrp error (not the waitpid errno) to the log, and
                    // then retry until not EPERM or the process group has exited.
                    FLOGF!(
                        proc_termowner,
                        "terminal_give_to_job(): EPERM with pgid %d.",
                        pgid
                    );
                    continue;
                }
            } else if errno::errno().0 == ENOTTY {
                // stdin is not a TTY. In general we expect this to be caught via the tcgetpgrp
                // call's EBADF handler above.
                return false;
            } else {
                FLOGF!(
                    warning,
                    wgettext!("Could not send job %d ('%s') with pgid %d to foreground"),
                    jg.job_id,
                    jg.command,
                    pgid
                );
                perror("tcsetpgrp");
                return false;
            }

            if pgroup_terminated {
                // All processes in the process group has exited.
                // Since we delay reaping any processes in a process group until all members of that
                // job/group have been started, the only way this can happen is if the very last
                // process in the group terminated and didn't need to access the terminal, otherwise
                // it would have hung waiting for terminal IO (SIGTTIN). We can safely ignore this.
                FLOGF!(
                    proc_termowner,
                    "tcsetpgrp called but process group %d has terminated.\n",
                    pgid
                );
                return false;
            }

            break;
        }
        true
    }
}

/// The destructor will assert if reclaim() has not been called.
impl Drop for TtyTransfer<'_> {
    fn drop(&mut self) {
        assert!(self.owner.is_none(), "Forgot to reclaim() the tty");
    }
}
