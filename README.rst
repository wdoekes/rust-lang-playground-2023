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

A function might look like this:

.. code-block::

    FIXME

- vars
- function sigs
- return values


Cargo.toml
----------

FIXME

- cargo add
- cargo build [--release]
- gitignore
- cargo new
- edition

