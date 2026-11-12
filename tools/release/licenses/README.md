# Bundled Linux runtime notices

The static Linux archives carry these complete upstream notices in
`THIRD-PARTY-LICENSES.md`, alongside the notices generated from Cargo and Rust.

The files were checked against the runtimes distributed with Rust 1.99.1:

- `musl-COPYRIGHT.txt` is the unmodified [musl 1.2.5 copyright file](https://git.musl-libc.org/cgit/musl/plain/COPYRIGHT?h=v1.2.5).
- `libunwind-LICENSE.txt` is the unmodified [LLVM libunwind license](https://github.com/rust-lang/llvm-project/blob/1b9c0d5ff9bbe7634aead059efe6b11a7eeba145/libunwind/LICENSE.TXT)
  from Rust's LLVM revision `1b9c0d5ff9bbe7634aead059efe6b11a7eeba145`.

When Rust changes its bundled runtimes, compare and refresh the notices and
these sources. Rust's [musl toolchain recipe](https://github.com/rust-lang/rust/blob/1.99.1/src/ci/docker/scripts/musl-toolchain.sh)
records the musl version. Its [LLVM submodule](https://github.com/rust-lang/rust/tree/1.99.1/src/llvm-project)
identifies the matching libunwind source.
