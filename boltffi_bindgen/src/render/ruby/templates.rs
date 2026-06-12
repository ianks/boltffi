use super::plan::*;
use askama::Template;

#[derive(Template)]
#[template(path = "render_ruby/lib.rb.txt", escape = "none")]
pub struct LibTemplate<'a> {
    pub crate_name: &'a str,
    pub module_name: &'a str,
    pub records: &'a [RubyRecord],
    pub enums: &'a [RubyEnum],
    pub functions: &'a [RubyFunction],
}

#[derive(Template)]
#[template(path = "render_ruby/extconf.rb.txt", escape = "none")]
pub struct ExtconfTemplate<'a> {
    pub crate_name: &'a str,
    pub lib_name: &'a str,
}

#[derive(Template)]
#[template(path = "render_ruby/gemspec.txt", escape = "none")]
pub struct GemspecTemplate<'a> {
    pub gem_name: &'a str,
    pub crate_name: &'a str,
    pub lib_name: &'a str, // staticlib stem → lib<lib_name>.a in the file manifest
    pub crate_version: &'a str, // from FfiContract.package.version, default "0.0.0"
}

#[derive(Template)]
#[template(path = "render_ruby/native.c.txt", escape = "none")]
pub struct NativeCTemplate<'a> {
    pub module_name: &'a str,
    pub crate_name: &'a str,
    pub functions: &'a [RubyNativeFunction],
    /// Deduplicated set of array element types that need a batched marshal helper.
    pub array_helpers: &'a [RubyArrayHelper],
    pub classes: &'a [RubyNativeClass], // opaque classes
    /// Emit `rb_ext_ractor_safe(true)` at the top of `Init_` (opt-in).
    pub ractor_safe: bool,
}

/// One entry per distinct array element type (deduplicated by the emitter).
/// Drives the `marshal_<name>_array` helper in native.c.txt.
#[derive(Debug, Clone)]
pub struct RubyArrayHelper {
    pub name: String,        // e.g. "i32", "f64"
    pub c_elem_type: String, // e.g. "int32_t", "double"
    pub vec_c_type: String,  // e.g. "FfiVecI32" — Rust Vec ABI struct name
    pub c_to_rb: String,     // per-element conversion, e.g. "RB_INT2NUM"
}

/// Flattened plan type for native.c.txt — all C-type strings pre-resolved.
#[derive(Debug, Clone)]
pub struct RubyNativeFunction {
    pub ruby_name: String,
    pub sym: String,
    pub c_return_type: String,
    pub c_to_rb: String,
    pub returns_void: bool, // void return — skip the `result` assignment (invalid C for void)
    pub returns_utf8_string: bool, // template uses rb_utf8_str_new + ENC_CODERANGE_VALID
    pub returns_bytes: bool, // template uses rb_str_new (ASCII-8BIT)
    pub params: Vec<RubyNativeParam>,
    pub param_count: usize, // passed to rb_define_module_function arity arg
}

#[derive(Debug, Clone)]
pub struct RubyNativeParam {
    pub ruby_name: String,
    pub c_type: String,
    pub rb_to_c: String,
    pub needs_type_check: bool, // emit Check_Type(arg, ruby_type_tag) guard
    pub ruby_type_tag: String,  // e.g. "T_STRING"
}

// ──────────────────────────────────────────────────────────────────────────────
// Opaque Classes — C glue for TypedData, constructors, sync methods
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RubyNativeClass {
    pub class_name: String,     // idiomatic Ruby (CamelCase)
    pub qualified_name: String, // "Module::ClassName"
    pub type_ident: String,     // C ident stem, e.g. "counter"
    pub c_struct_name: String,  // opaque C struct tag
    pub ffi_new: String,        // naming::class_ffi_new (symbol)
    pub ffi_free: String,       // naming::class_ffi_free (symbol)
    pub constructors: Vec<RubyNativeConstructor>,
    pub methods: Vec<RubyNativeMethod>,
    /// True if any method consumes `self` (by-value receiver). A consuming method
    /// nulls the handle after the call, so EVERY method must guard against a NULL
    /// handle — not just the consuming ones — to avoid dereferencing a freed handle.
    pub has_consuming_method: bool,
}

#[derive(Debug, Clone)]
pub struct RubyNativeConstructor {
    pub ruby_name: String, // "new" or factory method name
    pub ffi_sym: String,   // FFI symbol to call
    pub is_fallible: bool,
    pub params: Vec<RubyNativeParam>,
    pub param_count: usize,
}

#[derive(Debug, Clone)]
pub struct RubyNativeMethod {
    pub ruby_name: String,
    pub ffi_sym: String,
    pub params: Vec<RubyNativeParam>,
    pub param_count: usize,
    pub consumes_self: bool,
    pub is_fallible: bool,
    /// `Receiver::Static`: no TypedData receiver, registered as a singleton method.
    pub is_static: bool,
    /// Pre-rendered FFI call argument list. Instance methods lead with `handle`;
    /// static methods omit it. Keeps the comma/handle logic out of the template's
    /// five return-path branches.
    pub ffi_call_args: String,
    pub c_return_type: String, // for extern decl
    // Return type details — only one of these is used per method
    pub returns_void: bool,
    pub returns_utf8_string: bool,
    pub returns_bytes: bool,
    pub returns_handle: bool,              // true if Handle return
    pub returns_handle_type_ident: String, // type_ident if Handle return, empty otherwise
    pub scalar_c_to_rb: String,            // used if returns_scalar is true
    pub returns_scalar: bool,
    pub scalar_c_type: String, // c_type for scalar return
}
