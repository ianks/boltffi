use super::{plan::*, templates::*};
use askama::Template;

pub struct RubyOutputFile {
    pub relative_path: String,
    pub contents: String,
}

pub struct RubyPackageSources {
    pub files: Vec<RubyOutputFile>,
}

#[derive(Default)]
pub struct RubyEmitter {
    /// Emit `rb_ext_ractor_safe(true)` in `Init_` (opt-in via `[targets.ruby] ractor_safe`).
    ractor_safe: bool,
    /// Gem name override from `[targets.ruby] gem_name`. When unset, defaults to
    /// the crate stem with `_` → `-` (RubyGems' conventional separator).
    gem_name: Option<String>,
}

impl RubyEmitter {
    pub fn new(ractor_safe: bool, gem_name: Option<String>) -> Self {
        Self {
            ractor_safe,
            gem_name,
        }
    }

    pub fn emit(&self, module: &RubyModule) -> RubyPackageSources {
        // The crate stem (snake_case package name) drives every on-disk artifact
        // name; it is NOT the lowercased CamelCase module name (that would collapse
        // `HttpClient` → `httpclient`).
        let crate_name = module.crate_stem.clone();
        let gem_name = self
            .gem_name
            .clone()
            .unwrap_or_else(|| crate_name.replace('_', "-"));
        let lib_name = format!("{}_ffi", crate_name); // Rust staticlib stem → lib<lib_name>.a

        let native_fns: Vec<RubyNativeFunction> = module
            .functions
            .iter()
            .map(|f| self.to_native_function(f))
            .collect();

        let lib_rb = LibTemplate {
            crate_name: &crate_name,
            module_name: &module.name,
            records: &module.records,
            enums: &module.enums,
            functions: &module.functions,
        }
        .render()
        .expect("lib.rb render");

        let extconf = ExtconfTemplate {
            crate_name: &crate_name,
            lib_name: &lib_name,
        }
        .render()
        .expect("extconf.rb render");

        // Crate version is the single source of truth for the gem version; default to
        // "0.0.0" when the crate omits it (parity with the Python target's fallback).
        let crate_version = module.package_version.as_deref().unwrap_or("0.0.0");
        let gemspec = GemspecTemplate {
            gem_name: &gem_name,
            crate_name: &crate_name,
            lib_name: &lib_name,
            crate_version,
        }
        .render()
        .expect("gemspec render");

        // Collect deduplicated array helpers (one per distinct element RubyType)
        let array_helpers: Vec<RubyArrayHelper> = {
            // Dedup by the helper's C name (the identifier used in the generated typedef and
            // function), NOT by the Rust Debug repr of the element type.  Multiple distinct
            // RubyType variants (e.g. Record("Point") and Record("Color")) both collapse to
            // name="value" / c_elem_type="VALUE", so deduplication must happen on the C-level
            // name; otherwise we emit the same typedef/function multiple times and the C
            // compiler rejects the duplicate definitions.
            let mut seen = std::collections::HashSet::new();
            let mut helpers = Vec::new();
            for f in &module.functions {
                let all_types = f
                    .params
                    .iter()
                    .map(|p| &p.ruby_type)
                    .chain(std::iter::once(&f.return_type));
                for ty in all_types {
                    if let RubyType::Array(elem) = ty {
                        let helper = self.array_helper(elem);
                        if seen.insert(helper.name.clone()) {
                            helpers.push(helper);
                        }
                    }
                }
            }
            helpers
        };

        let native_classes: Vec<RubyNativeClass> = module
            .classes
            .iter()
            .map(|c| self.to_native_class(c))
            .collect();

        let native_c = NativeCTemplate {
            module_name: &module.name,
            crate_name: &crate_name,
            functions: &native_fns,
            array_helpers: &array_helpers,
            classes: &native_classes,
            ractor_safe: self.ractor_safe,
        }
        .render()
        .expect("native.c render");

        RubyPackageSources {
            files: vec![
                RubyOutputFile {
                    relative_path: format!("lib/{crate_name}.rb"),
                    contents: lib_rb,
                },
                RubyOutputFile {
                    relative_path: format!("ext/{crate_name}/extconf.rb"),
                    contents: extconf,
                },
                RubyOutputFile {
                    relative_path: format!("{gem_name}.gemspec"),
                    contents: gemspec,
                },
                RubyOutputFile {
                    relative_path: format!("ext/{crate_name}/_native.c"),
                    contents: native_c,
                },
            ],
        }
    }

    fn to_native_function(&self, f: &RubyFunction) -> RubyNativeFunction {
        let (c_return_type, c_to_rb, returns_utf8_string, returns_bytes) =
            self.c_return_info(&f.return_type);
        let params: Vec<RubyNativeParam> = f
            .params
            .iter()
            .map(|p| {
                let (c_type, rb_to_c, needs_type_check, ruby_type_tag) =
                    self.c_param_info(&p.ruby_type);
                RubyNativeParam {
                    ruby_name: p.ruby_name.clone(),
                    c_type,
                    rb_to_c,
                    needs_type_check,
                    ruby_type_tag: ruby_type_tag.to_string(),
                }
            })
            .collect();
        let param_count = params.len();
        RubyNativeFunction {
            ruby_name: f.ruby_name.clone(),
            sym: f.native_symbol.clone(),
            c_return_type,
            c_to_rb,
            returns_void: matches!(f.return_type, RubyType::Void),
            returns_utf8_string,
            returns_bytes,
            params,
            param_count,
        }
    }

    fn c_return_info(&self, ty: &RubyType) -> (String, String, bool, bool) {
        match ty {
            RubyType::Void => ("void".into(), "Qnil".into(), false, false),
            RubyType::Bool => (
                "int".into(),
                "_ffi_ret ? Qtrue : Qfalse".into(),
                false,
                false,
            ),
            RubyType::Integer { signed: true, bits } => (
                format!("int{bits}_t"),
                "rb_int2inum((intptr_t)_ffi_ret)".into(),
                false,
                false,
            ),
            RubyType::Integer {
                signed: false,
                bits,
            } => (
                format!("uint{bits}_t"),
                "rb_uint2inum((uintptr_t)_ffi_ret)".into(),
                false,
                false,
            ),
            RubyType::Float { bits: 32 } => (
                "float".into(),
                "rb_float_new((double)_ffi_ret)".into(),
                false,
                false,
            ),
            RubyType::Float { .. } => (
                "double".into(),
                "rb_float_new(_ffi_ret)".into(),
                false,
                false,
            ),
            // String / Bytes: c_to_rb is ignored; template uses the if/elsif branches instead.
            // BoltFFI string-returning functions return FfiBuf (an owned heap-allocated encoded
            // buffer with a 4-byte LE length prefix), NOT a bare FfiString/FfiBytes slice.
            RubyType::RubyString => ("FfiBuf".into(), "".into(), true, false),
            RubyType::Bytes => ("FfiBuf".into(), "".into(), false, true),
            // C-style (repr(i32)) enums cross the ABI boundary as int32_t
            RubyType::Enum(_) => (
                "int32_t".into(),
                "rb_int2inum((intptr_t)_ffi_ret)".into(),
                false,
                false,
            ),
            _ => ("VALUE".into(), "_ffi_ret".into(), false, false),
        }
    }

    fn c_param_info(&self, ty: &RubyType) -> (String, String, bool, &'static str) {
        match ty {
            RubyType::Bool => ("int".into(), "RB_TEST".into(), false, ""),
            // Use the exact-width C type for the extern parameter so it matches the
            // Rust ABI (a `u8` param declared as `int32_t` is an ABI mismatch). The
            // NUM2INT/NUM2UINT macros return `int`/`unsigned`, which C narrows to the
            // declared width on assignment; ≤32-bit ints fit in those macros.
            RubyType::Integer { signed: true, bits } => {
                let helper = if *bits <= 32 { "NUM2INT" } else { "NUM2LL" };
                (format!("int{bits}_t"), helper.into(), false, "")
            }
            RubyType::Integer {
                signed: false,
                bits,
            } => {
                let helper = if *bits <= 32 { "NUM2UINT" } else { "NUM2ULL" };
                (format!("uint{bits}_t"), helper.into(), false, "")
            }
            // NUM2DBL coerces + raises on non-numeric; the f32 extern param narrows double->float.
            RubyType::Float { bits: 32 } => ("float".into(), "NUM2DBL".into(), false, ""),
            RubyType::Float { .. } => ("double".into(), "NUM2DBL".into(), false, ""),
            // String/Bytes: Check_Type raises TypeError (Phase 1); RSTRING_PTR/LEN are safe after.
            // bolt_rb_to_ffi_string/bytes convert the validated VALUE to the FfiString/FfiBytes
            // struct expected by the Rust extern — no raw VALUE is ever passed to Rust.
            RubyType::RubyString => (
                "FfiString".into(),
                "bolt_rb_to_ffi_string".into(),
                true,
                "T_STRING",
            ),
            RubyType::Bytes => (
                "FfiBytes".into(),
                "bolt_rb_to_ffi_bytes".into(),
                true,
                "T_STRING",
            ),
            // C-style (repr(i32)) enums cross the ABI boundary as int32_t.
            RubyType::Enum(_) => ("int32_t".into(), "NUM2INT".into(), false, ""),
            _ => ("VALUE".into(), "".into(), false, ""),
        }
    }

    fn array_helper(&self, elem_ty: &RubyType) -> RubyArrayHelper {
        let (c_elem_type, c_to_rb, name) = match elem_ty {
            RubyType::Bool => ("int", "RB_TEST", "bool"),
            RubyType::Integer {
                signed: true,
                bits: 8,
            } => ("int8_t", "RB_INT2NUM", "i8"),
            RubyType::Integer {
                signed: true,
                bits: 16,
            } => ("int16_t", "RB_INT2NUM", "i16"),
            RubyType::Integer {
                signed: true,
                bits: 32,
            } => ("int32_t", "RB_INT2NUM", "i32"),
            RubyType::Integer {
                signed: true,
                bits: 64,
            }
            | RubyType::Integer {
                signed: true,
                bits: _,
            } => ("int64_t", "rb_int2inum", "i64"),
            RubyType::Integer {
                signed: false,
                bits: 8,
            } => ("uint8_t", "RB_UINT2NUM", "u8"),
            RubyType::Integer {
                signed: false,
                bits: 16,
            } => ("uint16_t", "RB_UINT2NUM", "u16"),
            RubyType::Integer {
                signed: false,
                bits: 32,
            } => ("uint32_t", "RB_UINT2NUM", "u32"),
            RubyType::Integer {
                signed: false,
                bits: 64,
            }
            | RubyType::Integer {
                signed: false,
                bits: _,
            } => ("uint64_t", "rb_uint2inum", "u64"),
            RubyType::Float { bits: 32 } => ("float", "rb_float_new", "f32"),
            RubyType::Float { .. } => ("double", "rb_float_new", "f64"),
            _ => ("VALUE", "", "value"),
        };

        RubyArrayHelper {
            name: name.to_string(),
            c_elem_type: c_elem_type.to_string(),
            vec_c_type: format!("FfiVec{}", name.to_uppercase()),
            c_to_rb: c_to_rb.to_string(),
        }
    }

    fn to_native_class(&self, class: &RubyClass) -> RubyNativeClass {
        let constructors = class
            .constructors
            .iter()
            .map(|ctor| {
                let params = ctor
                    .params
                    .iter()
                    .map(|p| {
                        let (c_type, rb_to_c, needs_type_check, ruby_type_tag) =
                            self.c_param_info(&p.ruby_type);
                        RubyNativeParam {
                            ruby_name: p.ruby_name.clone(),
                            c_type,
                            rb_to_c,
                            needs_type_check,
                            ruby_type_tag: ruby_type_tag.to_string(),
                        }
                    })
                    .collect::<Vec<_>>();
                let param_count = params.len();

                RubyNativeConstructor {
                    ruby_name: ctor.ruby_name.clone(),
                    ffi_sym: ctor.ffi_new.clone(),
                    is_fallible: ctor.is_fallible,
                    params,
                    param_count,
                }
            })
            .collect();

        let methods = class
            .methods
            .iter()
            .map(|method| {
                let params = method
                    .params
                    .iter()
                    .map(|p| {
                        let (c_type, rb_to_c, needs_type_check, ruby_type_tag) =
                            self.c_param_info(&p.ruby_type);
                        RubyNativeParam {
                            ruby_name: p.ruby_name.clone(),
                            c_type,
                            rb_to_c,
                            needs_type_check,
                            ruby_type_tag: ruby_type_tag.to_string(),
                        }
                    })
                    .collect::<Vec<_>>();
                let param_count = params.len();

                let (
                    returns_void,
                    returns_utf8_string,
                    returns_bytes,
                    returns_handle,
                    returns_handle_type_ident,
                    scalar_c_type,
                    scalar_c_to_rb,
                    returns_scalar,
                    c_return_type,
                ) = match &method.return_kind {
                    RubyReturnKind::Void => (
                        true,
                        false,
                        false,
                        false,
                        String::new(),
                        String::new(),
                        String::new(),
                        false,
                        "void".to_string(),
                    ),
                    RubyReturnKind::Handle(type_ident) => (
                        false,
                        false,
                        false,
                        true,
                        type_ident.clone(),
                        String::new(),
                        String::new(),
                        false,
                        "void*".to_string(),
                    ),
                    RubyReturnKind::Utf8String => (
                        false,
                        true,
                        false,
                        false,
                        String::new(),
                        String::new(),
                        String::new(),
                        false,
                        "FfiBuf".to_string(),
                    ),
                    RubyReturnKind::Bytes => (
                        false,
                        false,
                        true,
                        false,
                        String::new(),
                        String::new(),
                        String::new(),
                        false,
                        "FfiBuf".to_string(),
                    ),
                    RubyReturnKind::Scalar(ty) => {
                        let (c_type, c_to_rb, _, _) = self.c_return_info(ty);
                        (
                            false,
                            false,
                            false,
                            false,
                            String::new(),
                            c_type.clone(),
                            c_to_rb,
                            true,
                            c_type,
                        )
                    }
                };

                // Pre-render the FFI call argument list so the template's five
                // return-path branches share one expression. Instance methods lead
                // with the unwrapped `handle`; static methods omit it entirely.
                let arg_exprs = params
                    .iter()
                    .map(|p| format!("{}({})", p.rb_to_c, p.ruby_name));
                let ffi_call_args = if method.is_static {
                    arg_exprs.collect::<Vec<_>>().join(", ")
                } else {
                    std::iter::once("handle".to_string())
                        .chain(arg_exprs)
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                RubyNativeMethod {
                    ruby_name: method.ruby_name.clone(),
                    ffi_sym: method.ffi_sym.clone(),
                    params,
                    param_count,
                    consumes_self: method.consumes_self,
                    is_fallible: method.is_fallible,
                    is_static: method.is_static,
                    ffi_call_args,
                    c_return_type,
                    returns_void,
                    returns_utf8_string,
                    returns_bytes,
                    returns_handle,
                    returns_handle_type_ident,
                    scalar_c_to_rb,
                    returns_scalar,
                    scalar_c_type,
                }
            })
            .collect();

        let has_consuming_method = class.methods.iter().any(|m| m.consumes_self);

        RubyNativeClass {
            class_name: class.class_name.clone(),
            qualified_name: class.qualified_name.clone(),
            type_ident: class.type_ident.clone(),
            c_struct_name: class.c_struct_name.clone(),
            ffi_new: class
                .constructors
                .first()
                .map(|c| c.ffi_new.clone())
                .unwrap_or_default(),
            ffi_free: class.ffi_free.clone(),
            constructors,
            methods,
            has_consuming_method,
        }
    }
}
