use syscalls::{Sysno, syscall};

fn exit(n: usize) -> ! {
    unsafe {
        let _ignored_retval = syscall!(Sysno::exit, n);
        std::hint::unreachable_unchecked();
    }
}

fn write(fd: usize, buf: &[u8]) -> isize {
    let res; // or: let r: Result<usize, Errno>;
    unsafe {
        res = syscall!(Sysno::write, fd, buf.as_ptr(), buf.len());
    };
    let ret: isize;
    match res {
        Ok(val) => { ret = val as isize; }
        Err(_) => { ret = -1; },
    };
    ret
}

#[no_mangle]
pub fn any_name_except_main() {
    write(1, "Hello, world, using syscalls!\n".as_bytes());
    exit(0);
}

//#[no_mangle]
pub fn main() {
    write(1, "Hello, world, using syscalls!\n".as_bytes());
    exit(0);
}
