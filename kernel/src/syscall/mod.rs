//! System call

use alloc::{string::String, sync::Arc, vec::Vec};
use core::{slice, str, fmt};

use bitflags::bitflags;
use rcore_fs::vfs::{FileType, FsError, INode, Metadata};
use spin::{Mutex, MutexGuard};

use crate::arch::interrupt::TrapFrame;
use crate::fs::FileHandle;
use crate::process::*;
use crate::thread;
use crate::util;

use self::fs::*;
use self::mem::*;
use self::proc::*;
use self::time::*;
use self::ctrl::*;

mod fs;
mod mem;
mod proc;
mod time;
mod ctrl;

/// System call dispatcher
pub fn syscall(id: usize, args: [usize; 6], tf: &mut TrapFrame) -> isize {
    let ret = match id {
        // file
        000 => sys_read(args[0], args[1] as *mut u8, args[2]),
        001 => sys_write(args[0], args[1] as *const u8, args[2]),
        002 => sys_open(args[0] as *const u8, args[1], args[2]),
        003 => sys_close(args[0]),
        004 => sys_stat(args[0] as *const u8, args[1] as *mut Stat),
        005 => sys_fstat(args[0], args[1] as *mut Stat),
//        007 => sys_poll(),
        008 => sys_lseek(args[0], args[1] as i64, args[2] as u8),
        009 => sys_mmap(args[0], args[1], args[2], args[3], args[4] as i32, args[5]),
        011 => sys_munmap(args[0], args[1]),
        019 => sys_readv(args[0], args[1] as *const IoVec, args[2]),
        020 => sys_writev(args[0], args[1] as *const IoVec, args[2]),
//        021 => sys_access(),
        024 => sys_yield(),
        033 => sys_dup2(args[0], args[1]),
//        034 => sys_pause(),
        035 => sys_sleep(args[0]), // TODO: nanosleep
        039 => sys_getpid(),
//        040 => sys_getppid(),
//        041 => sys_socket(),
//        042 => sys_connect(),
//        043 => sys_accept(),
//        044 => sys_sendto(),
//        045 => sys_recvfrom(),
//        046 => sys_sendmsg(),
//        047 => sys_recvmsg(),
//        048 => sys_shutdown(),
//        049 => sys_bind(),
//        050 => sys_listen(),
//        054 => sys_setsockopt(),
//        055 => sys_getsockopt(),
//        056 => sys_clone(),
        057 => sys_fork(tf),
        059 => sys_exec(args[0] as *const u8, args[1] as usize, args[2] as *const *const u8, tf),
        060 => sys_exit(args[0] as isize),
        061 => sys_wait(args[0], args[1] as *mut i32), // TODO: wait4
        062 => sys_kill(args[0]),
//        072 => sys_fcntl(),
//        074 => sys_fsync(),
//        076 => sys_trunc(),
//        077 => sys_ftrunc(),
        078 => sys_getdirentry(args[0], args[1] as *mut DirEntry),
//        079 => sys_getcwd(),
//        080 => sys_chdir(),
//        082 => sys_rename(),
//        083 => sys_mkdir(),
//        086 => sys_link(),
//        087 => sys_unlink(),
        096 => sys_get_time(), // TODO: sys_gettimeofday
//        097 => sys_getrlimit(),
//        098 => sys_getrusage(),
//        133 => sys_mknod(),
        141 => sys_set_priority(args[0]),
//        160 => sys_setrlimit(),
//        162 => sys_sync(),
//        169 => sys_reboot(),
//        293 => sys_pipe(),

        // for musl: empty impl
        012 => {
            warn!("sys_brk is unimplemented");
            Ok(0)
        }
        013 => {
            warn!("sys_sigaction is unimplemented");
            Ok(0)
        }
        014 => {
            warn!("sys_sigprocmask is unimplemented");
            Ok(0)
        }
        016 => {
            warn!("sys_ioctl is unimplemented");
            Ok(0)
        }
        102 => {
            warn!("sys_getuid is unimplemented");
            Ok(0)
        }
        107 => {
            warn!("sys_geteuid is unimplemented");
            Ok(0)
        }
        108 => {
            warn!("sys_getegid is unimplemented");
            Ok(0)
        }
        131 => {
            warn!("sys_sigaltstack is unimplemented");
            Ok(0)
        }
        158 => sys_arch_prctl(args[0] as i32, args[1], tf),
        218 => {
            warn!("sys_set_tid_address is unimplemented");
            Ok(thread::current().id() as isize)
        }
        231 => {
            warn!("sys_exit_group is unimplemented");
            sys_exit(args[0] as isize);
        }
        _ => {
            error!("unknown syscall id: {:#x?}, args: {:x?}", id, args);
            crate::trap::error(tf);
        }
    };
    match ret {
        Ok(code) => code,
        Err(err) => -(err as isize),
    }
}

pub type SysResult = Result<isize, SysError>;

#[allow(dead_code)]
#[repr(isize)]
#[derive(Debug)]
pub enum SysError {
    EUNDEF = 0,
    EPERM = 1,
    ENOENT = 2,
    ESRCH = 3,
    EINTR = 4,
    EIO = 5,
    ENXIO = 6,
    E2BIG = 7,
    ENOEXEC = 8,
    EBADF = 9,
    ECHILD = 10,
    EAGAIN = 11,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    ENOTBLK = 15,
    EBUSY = 16,
    EEXIST = 17,
    EXDEV = 18,
    ENODEV = 19,
    ENOTDIR = 20,
    EISDIR = 21,
    EINVAL = 22,
    ENFILE = 23,
    EMFILE = 24,
    ENOTTY = 25,
    ETXTBSY = 26,
    EFBIG = 27,
    ENOSPC = 28,
    ESPIPE = 29,
    EROFS = 30,
    EMLINK = 31,
    EPIPE = 32,
    EDOM = 33,
    ERANGE = 34,
    EDEADLK = 35,
    ENAMETOOLONG = 36,
    ENOLCK = 37,
    ENOSYS = 38,
    ENOTEMPTY = 39,
}

#[allow(non_snake_case)]
impl fmt::Display for SysError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}",
            match self {
                EPERM => "Operation not permitted",
                ENOENT => "No such file or directory",
                ESRCH => "No such process",
                EINTR => "Interrupted system call",
                EIO => "I/O error",
                ENXIO => "No such device or address",
                E2BIG => "Argument list too long",
                ENOEXEC => "Exec format error",
                EBADF => "Bad file number",
                ECHILD => "No child processes",
                EAGAIN => "Try again",
                ENOMEM => "Out of memory",
                EACCES => "Permission denied",
                EFAULT => "Bad address",
                ENOTBLK => "Block device required",
                EBUSY => "Device or resource busy",
                EEXIST => "File exists",
                EXDEV => "Cross-device link",
                ENODEV => "No such device",
                ENOTDIR => "Not a directory",
                EISDIR => "Is a directory",
                EINVAL => "Invalid argument",
                ENFILE => "File table overflow",
                EMFILE => "Too many open files",
                ENOTTY => "Not a typewriter",
                ETXTBSY => "Text file busy",
                EFBIG => "File too large",
                ENOSPC => "No space left on device",
                ESPIPE => "Illegal seek",
                EROFS => "Read-only file system",
                EMLINK => "Too many links",
                EPIPE => "Broken pipe",
                EDOM => "Math argument out of domain of func",
                ERANGE => "Math result not representable",
                EDEADLK => "Resource deadlock would occur",
                ENAMETOOLONG => "File name too long",
                ENOLCK => "No record locks available",
                ENOSYS => "Function not implemented",
                ENOTEMPTY => "Directory not empty",
                _ => "Unknown error",
            },
        )
    }
}
