//! Runs a command and prints `<wall-clock ms> <peak RSS KiB>` for it (Linux only).
//!
//!     rustc -O tools/peak_rss.rs -o target/peak_rss
//!     target/peak_rss <command> [args...]
//!
//! Why not measure from Python: on Linux a child's `ru_maxrss` starts from the
//! memory of the process that spawned it, so a large launcher (a Python
//! interpreter that has just written a 50 MB file) inflates every measurement.
//! This launcher stays at ~1–2 MB, which is the floor of what it can report.

use std::process::{Command, Stdio};
use std::time::Instant;

#[repr(C)]
struct Timeval {
    sec: i64,
    usec: i64,
}

/// `struct rusage` on 64-bit Linux.
#[repr(C)]
struct Rusage {
    utime: Timeval,
    stime: Timeval,
    maxrss: i64,
    rest: [i64; 13],
}

unsafe extern "C" {
    fn wait4(pid: i32, status: *mut i32, options: i32, usage: *mut Rusage) -> i32;
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        eprintln!("usage: peak_rss <command> [args...]");
        std::process::exit(2);
    };
    let start = Instant::now();
    let child = Command::new(cmd)
        .args(rest)
        .stdout(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot run {cmd}: {e}"));
    let mut status = 0;
    let mut usage = Rusage {
        utime: Timeval { sec: 0, usec: 0 },
        stime: Timeval { sec: 0, usec: 0 },
        maxrss: 0,
        rest: [0; 13],
    };
    // SAFETY: `status` and `usage` are valid for writes, `usage` has the
    // layout of `struct rusage` on 64-bit Linux, and the pid is our child,
    // which is reaped here and never waited on again.
    let pid = unsafe { wait4(child.id() as i32, &mut status, 0, &mut usage) };
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    if pid < 0 || status != 0 {
        eprintln!("command failed: {args:?} (status {status})");
        std::process::exit(1);
    }
    println!("{ms:.3} {}", usage.maxrss);
}
