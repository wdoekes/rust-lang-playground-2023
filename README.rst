rust-lang playground 2023
=========================

*Toying around a bit with rust-lang in 2023.*

Why `Rust <https://www.rust-lang.org/>`_? Because it promises the speed
of C/C++ with type/memory safety, without the runtime overhead of
golang. (Five or six threads for a simple *Hello world*...!?)

Sections:

* `Install`_
* `Variables and functions`_
* `Cargo.toml`_


Install
-------

Setting up *rust* on *Ubuntu/Jammy* as *normal user*::

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

*Why? Because the rustc supplied with Ubuntu is slightly outdated,
causing more headaches than stability.*

This will install files in ``~/.cargo``, like ``~/.cargo/bin/rustc`` and
``~/.cargo/registry/src/github.com-1ecc6299db9ec823/libc-0.2.142/src/lib.rs``.

After ensuring your ``PATH`` is up to date, you can now create ``hello.rs``:

.. code-block:: rust

    fn main() {
        println!("Hello World!");
    }

Compile and run:

.. code-block:: console

   $ rustc hello.rs
   $ ./hello
   Hello World!
   $ stat -c%s hello
   4249176

4MiB is rather large. We can make it a tad bit smaller:

.. code-block:: console

    $ rustc hello.rs --edition=2021 -C strip=symbols \
        -C lto=true -C opt-level=3 -C panic=abort
    $ stat -c%s hello
    301448
    $ ./hello
    Hello World!

Without the miscellaneous ``-C`` options, you'll get a (way) bigger binary.
Until release, you'll probably want to stick with the defaults. (But see
the use of `Cargo.toml`_ below.)

Note that adding ``-C panic=abort`` is less beneficial than it looks.  The
binary is linked against ``libgcc_s.so.1`` regardless, even when we're
not using stack unwinding on panic:

.. code-block:: console

    $ ldd hello
            linux-vdso.so.1 (0x00007ffe93d48000)
            libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007ff685c23000)
            libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007ff6859fb000)
            /lib64/ld-linux-x86-64.so.2 (0x00007ff685cb3000)

.. code-block:: console

    $ diff -pu <(nm -D hello-panic-unwind) <(nm -D hello-panic-abort)
    --- /dev/fd/63	2023-05-04 12:01:40.967163712 +0200
    +++ /dev/fd/62	2023-05-04 12:01:40.967163712 +0200
    @@ -48,14 +48,12 @@
                      U sysconf@GLIBC_2.2.5
                      U __tls_get_addr@GLIBC_2.3
                      U _Unwind_Backtrace@GCC_3.3
    -                 U _Unwind_DeleteException@GCC_3.0
                      U _Unwind_GetDataRelBase@GCC_3.0
                      U _Unwind_GetIP@GCC_3.0
                      U _Unwind_GetIPInfo@GCC_4.2.0
                      U _Unwind_GetLanguageSpecificData@GCC_3.0
                      U _Unwind_GetRegionStart@GCC_3.0
                      U _Unwind_GetTextRelBase@GCC_3.0
    -                 U _Unwind_RaiseException@GCC_3.0
                      U _Unwind_Resume@GCC_3.0
                      U _Unwind_SetGR@GCC_3.0
                      U _Unwind_SetIP@GCC_3.0

*Why am I so obsessed with size? Because I'd like to use programs not
only as microservices, but also as simple binaries. The concept of simple
programs taking up 4MiB is ridiculous to me.*


Variables and functions
-----------------------

For starters, this ``println!`` that we see is a *macro*, not a
function. Because functions take have a fixed *arity*, *macros* can be
used to support multiple arguments or differing argument types.

See `variadics <https://doc.rust-lang.org/rust-by-example/macros/variadics.html>`_.

Calling a function might look like this:

.. code-block:: rust

    fn add(a: u8, b: u8) -> u16 {
        // thread 'main' panicked at 'attempt to add with overflow'
        // run with `RUST_BACKTRACE=full` for a verbose backtrace
        //let c: u16 = (a + b) as u16;
        let c: u16 = (a as u16) + (b as u16);
        // We can do an explicit return
        return c;
        // Otherwise the last statement without semi-colon is the return value
        0xbeef
    }

    fn main() -> () {
        // See: https://doc.rust-lang.org/rust-by-example/hello/print.html
        println!("add = 0x{:x}", add(255, 255));
        // Exit with something other than 0?
        std::process::exit(1)
    }

.. code-block:: console

    $ rustc func.rs
    $ ./func
    add = 0x1fe

What we've also learnt here, is that we want to set
``RUST_BACKTRACE=full`` in the environment when running microservices.
We do want full backtraces if something goes wrong.


Cargo.toml
----------

For projects that are not toy examples, we'll use ``cargo`` and a
``Cargo.toml`` file.

Use ``cargo new`` to set up a directory:

.. code-block:: console

    $ cargo new helloproj
         Created binary (application) `helloproj` package

.. code-block:: console

    $ find helloproj/ -type f
    helloproj/Cargo.toml
    helloproj/src/main.rs

This includes the *Hello world* app we saw earlier and a ``Cargo.toml``
that looks like this:

.. code-block:: toml

    [package]
    name = "helloproj"
    version = "0.1.0"
    edition = "2021"

    [dependencies]

This ``edition`` setting is important. Don't omit it.

.. code-block:: console

    $ cd helloproj
    $ cargo build
       Compiling helloproj v0.1.0 (rust-lang-playground-2023/helloproj)
        Finished dev [unoptimized + debuginfo] target(s) in 0.33s
    $ ./target/debug/helloproj
    Hello, world!

Setting default optimization options for the ``--release`` build in
``Cargo.toml``:

.. code-block:: toml

    [profile.release]
    strip = true        # Automatically strip symbols from the binary
                        # (don't use for microservices, you want backtraces)
    #opt-level = "z"    # Optimize for size?
    lto = true          # Enable Link Time Optimization (LTO)
    codegen-units = 1   # serial build, slow, but better opt
    #panic = "abort"    # No debug stacktrace awesomeness?

Now we build using ``cargo build --release``. The output is at
``./target/release/helloproj``.


Dependencies
------------

Let's do this again, creating ``helloasm``, but now we create a library instead.

We reimplement parts of `151-byte static Linux binary in Rust
<http://mainisusuallyafunction.blogspot.com/2015/01/151-byte-static-linux-binary-in-rust.html>`_
(did I mention I like small things?), just to get a feel of *Rust* low level internals.

- Cargo.toml
- build
- Makefile

While still in the ``helloasm`` directory, we can add some dependencies:

.. code-block:: console

    $ cargo add syscalls
        Updating crates.io index
          Adding syscalls v0.6.10 to dependencies.

.. code-block:: console

    $ tail -n2 Cargo.toml
    [dependencies]
    syscalls = "0.6.10"

We alter ``main.rs`` to ``lib.rs``:

.. code-block:: rust

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
    pub fn main() {
        write(1, "Hello, world!\n".as_bytes());
        exit(0);
    }

We set the project output type to ``rlib`` in ``Cargo.toml``:

.. code-block:: toml

    [lib]
    crate-type = ["rlib"]

I added a small ``Makefile`` for convenience. Letting us fetch ``main.o``:

.. code-block:: console

    $ make
    cargo build --release
       Compiling helloasm v0.1.0 (/home/walter/srcelf/rust-lang-playground-2023/helloasm)
        Finished release [optimized] target(s) in 0.09s
    f=$(ar t target/release/libhelloasm.rlib | grep -vxF lib.rmeta) && \
      ar x target/release/libhelloasm.rlib "$f" && \
      mv "$f" main.o

.. code-block:: console

    $ objdump -dr main.o
    ...

    0000000000000000 <main>:
       0:	48 8d 35 00 00 00 00 	lea    0x0(%rip),%rsi        # 7 <main+0x7>
                            3: R_X86_64_PC32	.rodata..Lanon.fad58de7366495db4650cfefac2fcd61.0-0x4
       7:	b8 01 00 00 00       	mov    $0x1,%eax
       c:	bf 01 00 00 00       	mov    $0x1,%edi
      11:	ba 0e 00 00 00       	mov    $0xe,%edx
      16:	0f 05                	syscall
      18:	b8 3c 00 00 00       	mov    $0x3c,%eax
      1d:	31 ff                	xor    %edi,%edi
      1f:	0f 05                	syscall
      21:	0f 0b                	ud2
