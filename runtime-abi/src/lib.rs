//! The native runtime's C ABI, written once. `runtime_fns!` is the table; `src/emit_llvm.rs`
//! expands it into the LLVM declarations and `runtime-rs` expands it into a compile-time check
//! that every entry is defined with exactly this signature. Same shape as Swift's
//! RuntimeFunctions.def x-macro file (plans/native-runtime-rust-research.md, section 3.4).
//!
//! Adding a runtime function: add its entry to the table, write the `extern "C"` definition in
//! runtime-rs. The compiler picks the entry up as a field of the same name; a definition that
//! disagrees with the entry does not compile.

/// The types a runtime function's signature can use, as LLVM sees them. `Ptr` is any raw pointer
/// on the Rust side: the LLVM IR uses opaque pointers, so the pointee is not part of the ABI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Ptr,
    I64,
    I32,
    F64,
    /// No return value. Also what a `-> !` function looks like from LLVM.
    Void,
}

/// One table entry as data: what the definition is measured against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sig {
    pub params: &'static [Ty],
    pub ret: Ty,
}

/// A Rust type that stands for a `Ty` in a definition's signature.
pub trait AbiTy {
    const TY: Ty;
}

impl<T> AbiTy for *const T {
    const TY: Ty = Ty::Ptr;
}
impl<T> AbiTy for *mut T {
    const TY: Ty = Ty::Ptr;
}
impl AbiTy for i64 {
    const TY: Ty = Ty::I64;
}
impl AbiTy for i32 {
    const TY: Ty = Ty::I32;
}
impl AbiTy for f64 {
    const TY: Ty = Ty::F64;
}
impl AbiTy for () {
    const TY: Ty = Ty::Void;
}

/// A function pointer whose signature can be read off as a `Sig`. Implemented for the arities the
/// runtime uses, in the `unsafe extern "C"` form every definition coerces to. A function with more
/// parameters fails the check with "trait bound not satisfied": add an arity here.
pub trait AbiFn {
    const SIG: Sig;
}

macro_rules! impl_abi_fn {
    ($($arg:ident),*) => {
        impl<$($arg: AbiTy,)* R: AbiTy> AbiFn for unsafe extern "C" fn($($arg),*) -> R {
            const SIG: Sig = Sig { params: &[$($arg::TY),*], ret: R::TY };
        }
        // `!` cannot implement `AbiTy` on stable, so a function that does not return gets its
        // own impl. LLVM sees it as returning void.
        impl<$($arg: AbiTy),*> AbiFn for unsafe extern "C" fn($($arg),*) -> ! {
            const SIG: Sig = Sig { params: &[$($arg::TY),*], ret: Ty::Void };
        }
    };
}
impl_abi_fn!();
impl_abi_fn!(A);
impl_abi_fn!(A, B);
impl_abi_fn!(A, B, C);
impl_abi_fn!(A, B, C, D);
impl_abi_fn!(A, B, C, D, E);

pub const fn sig_of<F: AbiFn + Copy>(_: F) -> Sig {
    F::SIG
}

/// Structural equality in a const context, where `PartialEq` is not callable.
pub const fn sig_eq(a: Sig, b: Sig) -> bool {
    if a.ret as u8 != b.ret as u8 || a.params.len() != b.params.len() {
        return false;
    }
    let mut i = 0;
    while i < a.params.len() {
        if a.params[i] as u8 != b.params[i] as u8 {
            return false;
        }
        i += 1;
    }
    true
}

/// The vocabulary word for a type, as a `Ty`. The table is written in these words.
#[macro_export]
macro_rules! ty {
    (ptr) => {
        $crate::Ty::Ptr
    };
    (i64) => {
        $crate::Ty::I64
    };
    (i32) => {
        $crate::Ty::I32
    };
    (f64) => {
        $crate::Ty::F64
    };
    (void) => {
        $crate::Ty::Void
    };
    (never) => {
        $crate::Ty::Void
    };
}

/// Calls `$callback! { fn name(arg: ty, ...) -> ret; ... }` with every runtime function, in the
/// order the compiler declares them in the LLVM module (which is the order the emitted IR lists
/// them in). The vocabulary is `ptr`, `i64`, `i32`, `f64` and, for a return type, `void`, or
/// `never` for a function that does not return.
#[macro_export]
macro_rules! runtime_fns {
    ($callback:path) => {
        $callback! {
            fn tl_concat(a: ptr, b: ptr) -> ptr;
            fn tl_int_to_str(n: i64) -> ptr;
            fn tl_float_to_str(x: f64) -> ptr;
            fn tl_str_eq(a: ptr, b: ptr) -> i64;
            fn tl_str_cmp(a: ptr, b: ptr) -> i64;
            fn tl_print(s: ptr) -> void;
            fn tl_quote(s: ptr) -> ptr;
            fn tl_str_join(parts: ptr, open: ptr, sep: ptr, close: ptr) -> ptr;
            fn tl_vec_new(len: i64, ncols: i64) -> ptr;
            fn tl_vec_len(v: ptr) -> i64;
            fn tl_vec_get(v: ptr, col: i64, i: i64) -> i64;
            fn tl_vec_set(v: ptr, col: i64, i: i64, value: i64) -> void;
            fn tl_vec_from_mask(src: ptr, keep: ptr) -> ptr;
            fn tl_mask_new(len: i64) -> ptr;
            fn tl_mask_set(mask: ptr, i: i64, value: i64) -> void;
            fn tl_sel_len(src: ptr, keep: ptr) -> i64;
            fn tl_sel_at(src: ptr, keep: ptr, i: i64, is_record: i32) -> ptr;
            fn tl_vec_column(v: ptr, col: i64) -> ptr;
            fn tl_rec_get(r: ptr, field: i64) -> i64;
            fn tl_rec_new(nfields: i64) -> ptr;
            fn tl_collect_lines() -> ptr;
            fn tl_split_lines(lines: ptr, sep: ptr) -> ptr;
            fn tl_rec_set(r: ptr, field: i64, value: i64) -> void;
            fn tl_read_input(descriptor: ptr) -> i64;
            fn tl_parse_str(value: ptr, descriptor: ptr) -> i64;
            fn tl_read_inputs(descriptor: ptr) -> ptr;
            fn tl_read_one_input(descriptor: ptr, out: ptr) -> i32;
            fn tl_read_one_line(out: ptr) -> i32;
            fn tl_rec_from_vec(v: ptr, i: i64) -> ptr;
            fn tl_at(v: ptr, i: i64, depth: i64, is_record: i32) -> ptr;
            fn tl_opt_is_some(o: ptr) -> i64;
            fn tl_opt_get(o: ptr) -> i64;
            fn tl_opt_some(value: i64) -> ptr;
            fn tl_unwrap(o: ptr, depth: i64) -> ptr;
            fn tl_div_by_zero() -> never;
            fn tl_range(n: i64) -> ptr;
            fn tl_chars(s: ptr) -> ptr;
            fn tl_vec_tail(v: ptr) -> ptr;
            fn tl_vec_first(v: ptr, is_record: i32) -> ptr;
            fn tl_vec_any(v: ptr) -> i64;
            fn tl_vec_all(v: ptr) -> i64;
            fn tl_vec_flatten(vv: ptr, ncols: i64) -> ptr;
            fn tl_vec_slice(v: ptr, lo: i64, hi: i64, depth: i64) -> ptr;
            fn tl_vec_concat(a: ptr, b: ptr, ncols: i64) -> ptr;
            fn tl_vec_sort_int(v: ptr) -> ptr;
            fn tl_vec_sort_str(v: ptr) -> ptr;
            fn tl_vec_reverse(v: ptr, ncols: i64) -> ptr;
            fn tl_vec_sum(v: ptr, narrow: i32) -> i64;
            fn tl_vec_transpose(vv: ptr, ncols: i64) -> ptr;
            fn tl_pipe_through(cmd: ptr, args: ptr, lines: ptr, stdout_tag: i64, stderr_tag: i64) -> ptr;
            fn tl_vec_max(v: ptr) -> ptr;
            fn tl_vec_sort_by(v: ptr, keys: ptr, is_str: i32) -> ptr;
            fn tl_vec_max_by(v: ptr, keys: ptr, is_str: i32, want_max: i32) -> ptr;
        }
    };
}

/// Expands to a compile-time check that every table entry names a function in scope whose
/// signature is the entry's. Invoke it once in runtime-rs, where every definition is in scope.
#[macro_export]
macro_rules! assert_defined_as_declared {
    () => {
        $crate::runtime_fns!($crate::__assert_entries);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __assert_entries {
    ($(fn $name:ident($($arg:ident: $ty:ident),*) -> $ret:ident;)*) => {
        $(
            const _: () = assert!(
                $crate::__entry_matches!($name, [$($ty),*], $ret),
                concat!("`", stringify!($name), "` is defined with a signature other than its table entry's"),
            );
        )*
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __infer {
    ($_arg:ident) => {
        _
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __entry_matches {
    ($name:ident, [$($ty:ident),*], $ret:ident) => {
        $crate::sig_eq(
            $crate::sig_of($name as unsafe extern "C" fn($($crate::__infer!($ty)),*) -> _),
            $crate::Sig { params: &[$($crate::ty!($ty)),*], ret: $crate::ty!($ret) },
        )
    };
}
