// Implementation of the fg builtin.

use super::shared::{
    builtin_print_error_trailer, builtin_print_help, io_streams_t, BUILTIN_ERR_NOT_NUMBER,
    STATUS_CMD_ERROR, STATUS_INVALID_ARGS,
};
use crate::{
    builtins::shared::{HelpOnlyCmdOpts, STATUS_CMD_OK},
    env::EnvMode,
    fds::make_fd_blocking,
    ffi::{self, parser_t, reader_write_title, Repin},
    proc::TtyTransfer,
    tokenizer::tok_command,
    wchar::{wstr, L},
    wchar_ffi::{WCharFromFFI, WCharToFFI},
    wutil::{fish_wcstoi, perror, wgettext_fmt},
};
use libc::{c_int, tcsetattr, STDIN_FILENO, TCSADRAIN};
use std::sync::{Arc, RwLock};

/// Builtin for putting a job in the foreground.
pub fn fg(parser: &mut parser_t, streams: &mut io_streams_t, args: &mut [&wstr]) -> Option<c_int> {
    let opts = match HelpOnlyCmdOpts::parse(args, parser, streams) {
        Ok(opts) => opts,
        Err(err @ Some(_)) if err != STATUS_CMD_OK => return err,
        Err(err) => panic!("Illogical exit code from parse_options(): {err:?}"),
    };

    let cmd = args[0];
    if opts.print_help {
        builtin_print_help(parser, streams, args.get(0)?);
        return STATUS_CMD_OK;
    }

    let job = if opts.optind == args.len() {
        // Select last constructed job (i.e. first job in the job queue) that can be brought
        // to the foreground.
        let jobs = parser.get_jobs();
        let job_pos = jobs.iter().position(|job| {
            if let Some(job) = job.as_ref() {
                return job.is_stopped() && job.wants_job_control() && !job.is_completed();
            }

            false
        });

        let Some(job_pos) = job_pos else {
            streams
                    .err
                    .append(wgettext_fmt!("%ls: There are no suitable jobs\n", cmd));
                return STATUS_CMD_ERROR;
        };

        &parser.get_jobs()[job_pos]
    } else if opts.optind + 1 < args.len() {
        // Specifying more than one job to put to the foreground is a syntax error, we still
        // try to locate the job $argv[1], since we need to determine which error message to
        // emit (ambigous job specification vs malformed job id).
        let mut found_job = false;
        let pid = fish_wcstoi(args[opts.optind]);
        if pid.is_ok() && pid.unwrap() > 0 {
            found_job = parser.job_get_from_pid(pid.unwrap()).is_some();
        }

        if found_job {
            streams
                .err
                .append(wgettext_fmt!("%ls: Ambiguous job\n", cmd));
        } else {
            streams.err.append(wgettext_fmt!(
                "%ls: '%ls' is not a job\n",
                cmd,
                args[opts.optind]
            ));
        }

        builtin_print_error_trailer(parser, streams, cmd);
        return STATUS_CMD_ERROR;
    } else {
        let pid = fish_wcstoi(args[opts.optind]);
        if pid.is_err() {
            streams.err.append(wgettext_fmt!(
                BUILTIN_ERR_NOT_NUMBER,
                cmd,
                args[opts.optind]
            ));
            builtin_print_error_trailer(parser, streams, cmd);
            return STATUS_INVALID_ARGS;
        } else {
            let job = parser.job_get_from_pid(pid.unwrap());
            if job.is_some() || !job.unwrap().is_constructed() || !job.unwrap().is_completed() {
                streams.err.append(wgettext_fmt!(
                    "%ls: No suitable job: %d\n",
                    cmd,
                    pid.unwrap()
                ));
                return STATUS_INVALID_ARGS;
            } else if !job.unwrap().wants_job_control() {
                streams.err.append(wgettext_fmt!("%ls: Can't put job %d, '%ls' to foreground because it is not under job control\n",
                                          cmd, pid.unwrap(), job.unwrap().command().from_ffi()));
                return STATUS_INVALID_ARGS;
            }
            job.unwrap()
        }
    };

    let commandline = job.command().from_ffi();

    let output = wgettext_fmt!(
        "Send job %d (%s) to foreground\n",
        i32::from(job.job_id()),
        commandline
    );

    if streams.err_is_redirected {
        streams.err.append(output);
    } else {
        // If we aren't redirecting, send output to real stderr, since stuff in sb_err won't get
        // printed until the command finishes.
        eprint!("{}", output);
    }

    // job holds an immutable reference to the parser open, so grab its position instead
    // Can't do this in the find-job phase (like in bg) as too much metadata from the job is
    // required
    let job_pos = parser
        .get_jobs()
        .iter()
        .position(|some_job| {
            some_job.as_ref().unwrap().get_internal_job_id() == job.get_internal_job_id()
        })
        .unwrap();

    let command = tok_command(&commandline);
    if !command.is_empty() {
        // Provide value for `status current-command`
        // Provide value for `status current-commandline`
        parser
            .pin()
            .libdata()
            .set_status_vars_ffi(&command, &commandline);
        // Also provide a value for the deprecated fish 2.0 $_ variable
        parser
            .pin()
            .set_var_and_fire(&L!("_").to_ffi(), EnvMode::EXPORT.bits(), &commandline);
    }

    reader_write_title(&commandline.to_ffi(), parser.pin(), true);

    // Note if tty transfer fails, we still try running the job.
    parser.pin().job_promote_at(job_pos);

    let _ = make_fd_blocking(STDIN_FILENO);

    // Get the job object back
    let job = &parser.get_jobs()[job_pos];

    let mut job_group = unsafe {
        std::mem::transmute::<&ffi::job_group_t, &crate::job_group::JobGroup>(job.ffi_group())
    };
    job_group.set_is_foreground(true);
    if job_group.wants_terminal() && job_group.tmodes.is_some() {
        let termios = job_group.tmodes.unwrap();
        let res = unsafe { tcsetattr(STDIN_FILENO, TCSADRAIN, &termios) };
        if res != 0 {
            perror("tcsetattr");
        }
    }
    let mut transfer = TtyTransfer::new();
    //let job_group_ref = Arc::new(RwLock::new(job_group));
    //transfer.to_job_group(&job_group_ref);
    transfer.to_job_group(job_group);
    let resumed = job.ffi_resume();
    if resumed {
        //job.continue_job(parser);
    }
    if job.is_stopped() {
        transfer.save_tty_modes();
    }
    transfer.reclaim();

    match resumed {
        true => STATUS_CMD_OK,
        false => STATUS_CMD_ERROR,
    }
}
