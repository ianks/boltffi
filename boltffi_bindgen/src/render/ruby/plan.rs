#[derive(Debug, Clone)]
pub struct RubyModule {
    pub name: String, // CamelCase module name (from crate name)
    /// Cargo crate stem — the package name normalized to snake_case (`-` → `_`).
    /// Drives every on-disk artifact name (`lib/<stem>.rb`, `ext/<stem>/`,
    /// `<stem>_native.so`, `Init_<stem>_native`). Derived from the raw package
    /// name, NOT by lowercasing the CamelCase module name — lowercasing collapses
    /// word boundaries (`HttpClient` → `httpclient` instead of `http_client`).
    pub crate_stem: String,
    pub package_version: Option<String>, // from FfiContract.package.version → gemspec
    pub records: Vec<RubyRecord>,
    pub enums: Vec<RubyEnum>,
    pub functions: Vec<RubyFunction>,
    pub classes: Vec<RubyClass>, // opaque Rust heap objects
}

#[derive(Debug, Clone)]
pub struct RubyRecord {
    pub class_name: String,
    pub fields: Vec<RubyField>,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct RubyField {
    pub ruby_name: String, // snake_case
    pub ruby_type: RubyType,
}

#[derive(Debug, Clone)]
pub struct RubyEnum {
    pub class_name: String,
    pub variants: Vec<RubyEnumVariant>,
}

#[derive(Debug, Clone)]
pub struct RubyEnumVariant {
    pub const_name: String, // SCREAMING_SNAKE
    pub discriminant: i128,
}

#[derive(Debug, Clone)]
pub struct RubyFunction {
    pub ruby_name: String,
    pub params: Vec<RubyParam>,
    pub return_type: RubyType,
    pub native_symbol: String,
}

#[derive(Debug, Clone)]
pub struct RubyParam {
    pub ruby_name: String,
    pub ruby_type: RubyType,
}

#[derive(Debug, Clone)]
pub enum RubyType {
    Void,
    Bool,
    Integer { signed: bool, bits: u8 },
    Float { bits: u8 },
    RubyString,
    Record(String), // class name
    Enum(String),   // class name
    Bytes,
    Array(Box<RubyType>),
    Handle(String), // opaque class by type_ident
}

// ──────────────────────────────────────────────────────────────────────────────
// Opaque Classes — TypedData, constructors, sync methods, GC free
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RubyClass {
    pub class_name: String,     // idiomatic Ruby (heck UpperCamel)
    pub qualified_name: String, // "Demo::Counter" — the TypedData wrap_struct_name
    pub type_ident: String,     // C ident stem, e.g. "counter" → c_Counter, counter_type
    pub c_struct_name: String,  // opaque C struct tag (matches Rust type name)
    pub ffi_free: String,       // naming::class_ffi_free(rust_name) — e.g. boltffi_counter_free
    pub constructors: Vec<RubyConstructor>,
    pub methods: Vec<RubyMethod>,
}

#[derive(Debug, Clone)]
pub struct RubyConstructor {
    pub ruby_name: String, // "new" (factory) or a named ctor
    pub params: Vec<RubyParam>,
    pub ffi_new: String,   // naming::class_ffi_new / method_ffi_name
    pub is_fallible: bool, // Result<Self,E> → null + last_error channel
}

#[derive(Debug, Clone)]
pub struct RubyMethod {
    pub ruby_name: String,
    pub params: Vec<RubyParam>,      // receiver is implicit (self) unless `is_static`
    pub return_kind: RubyReturnKind, // see plan § C
    pub ffi_sym: String,             // naming::method_ffi_name(class, method)
    pub consumes_self: bool,         // by-value receiver → invalidate handle after call
    pub is_fallible: bool,
    pub is_static: bool, // `Receiver::Static` (no `self`) → bound as a singleton method
}

#[derive(Debug, Clone)]
pub enum RubyReturnKind {
    Void,             // -> FfiStatus OK sentinel
    Scalar(RubyType), // primitive scalar returned directly
    Handle(String),   // returns another opaque class (type_ident)
    Utf8String,
    Bytes,
}
