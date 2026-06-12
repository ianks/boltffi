use super::naming;
use super::plan::*;
use crate::ir::abi::CallId;
use crate::ir::definitions::{ClassDef, ConstructorDef, Receiver, ReturnDef};
use crate::ir::ids::ClassId;
use crate::ir::types::{PrimitiveType, TypeExpr};
use crate::ir::{AbiContract, FfiContract};
use boltffi_ffi_rules::naming::{
    class_ffi_free, class_ffi_new, function_ffi_name, method_ffi_name,
};

pub struct RubyLowerer<'a> {
    ffi_contract: &'a FfiContract,
    abi_contract: &'a AbiContract,
}

impl<'a> RubyLowerer<'a> {
    pub fn new(ffi_contract: &'a FfiContract, abi_contract: &'a AbiContract) -> Self {
        Self {
            ffi_contract,
            abi_contract,
        }
    }

    pub fn lower(self) -> RubyModule {
        RubyModule {
            // FfiContract exposes package metadata via `package` (see ir/contract.rs:23,
            // PackageInfo { name, version }) — NOT a flat `crate_name`.
            name: naming::class_name(&self.ffi_contract.package.name),
            // Cargo normalizes crate names by replacing `-` with `_`; mirror that
            // so the gem's file layout matches the crate, and DON'T derive this by
            // lowercasing the CamelCase module name (that collapses word boundaries).
            crate_stem: self
                .ffi_contract
                .package
                .name
                .replace('-', "_")
                .to_ascii_lowercase(),
            package_version: self.ffi_contract.package.version.clone(),
            records: self.lower_records(),
            enums: self.lower_enums(),
            functions: self.lower_functions(),
            classes: self.lower_classes(),
        }
    }

    fn lower_records(&self) -> Vec<RubyRecord> {
        self.abi_contract
            .records
            .iter()
            .filter_map(|abi_rec| {
                self.ffi_contract
                    .catalog
                    .resolve_record(&abi_rec.id)
                    .map(|ffi_rec| RubyRecord {
                        class_name: naming::class_name(abi_rec.id.as_str()),
                        fields: ffi_rec
                            .fields
                            .iter()
                            .map(|f| RubyField {
                                ruby_name: naming::method_name(f.name.as_str()),
                                ruby_type: self.lower_type(&f.type_expr),
                            })
                            .collect(),
                        is_error: ffi_rec.is_error,
                    })
            })
            .collect()
    }

    fn lower_enums(&self) -> Vec<RubyEnum> {
        self.abi_contract
            .enums
            .iter()
            .filter_map(|abi_enum| {
                self.ffi_contract
                    .catalog
                    .resolve_enum(&abi_enum.id)
                    .map(|_ffi_enum| RubyEnum {
                        class_name: naming::class_name(abi_enum.id.as_str()),
                        variants: abi_enum
                            .variants
                            .iter()
                            .map(|v| RubyEnumVariant {
                                const_name: naming::const_name(v.name.as_str()),
                                discriminant: v.discriminant,
                            })
                            .collect(),
                    })
            })
            .collect()
    }

    fn lower_functions(&self) -> Vec<RubyFunction> {
        self.abi_contract
            .calls
            .iter()
            .filter_map(|call| {
                match &call.id {
                    CallId::Function(func_id) => {
                        self.ffi_contract
                            .functions
                            .iter()
                            // Async free functions are not yet supported: their exported
                            // symbol is a future-handle poll entry, not a sync call, so they
                            // must not be lowered as ordinary functions.
                            .find(|f| f.id == *func_id && !f.is_async())
                            // Only lower free functions whose params and return
                            // the function template can actually marshal. A function
                            // with a composite/handle param or a composite/Result/handle
                            // return would otherwise emit glue with a signature that
                            // doesn't match the exported ABI, so skip it entirely.
                            .filter(|ffi_func| {
                                ffi_func
                                    .params
                                    .iter()
                                    .all(|p| Self::is_supported_param(&p.type_expr))
                                    && Self::is_supported_function_return(&ffi_func.returns)
                            })
                            .map(|ffi_func| {
                                RubyFunction {
                                    ruby_name: naming::method_name(ffi_func.id.as_str()),
                                    params: ffi_func
                                        .params
                                        .iter()
                                        .map(|p| RubyParam {
                                            ruby_name: naming::method_name(p.name.as_str()),
                                            ruby_type: self.lower_type(&p.type_expr),
                                        })
                                        .collect(),
                                    return_type: match &ffi_func.returns {
                                        ReturnDef::Void => RubyType::Void,
                                        ReturnDef::Value(ty) => self.lower_type(ty),
                                        ReturnDef::Result { .. } => {
                                            // Result returns are not yet supported for free functions.
                                            RubyType::Void
                                        }
                                    },
                                    native_symbol: function_ffi_name(ffi_func.id.as_str())
                                        .to_string(),
                                }
                            })
                    }
                    _ => None,
                }
            })
            .collect()
    }

    fn lower_classes(&self) -> Vec<RubyClass> {
        self.ffi_contract
            .catalog
            .all_classes()
            .map(|class_def| {
                let class_name = naming::class_name(class_def.id.as_str());
                let type_ident = naming::method_name(class_def.id.as_str());
                let qualified_name = format!("{}::{}", self.ffi_contract.package.name, class_name);
                let c_struct_name = class_def.id.as_str().to_string();
                let ffi_free = class_ffi_free(class_def.id.as_str()).to_string();

                RubyClass {
                    class_name,
                    qualified_name,
                    type_ident,
                    c_struct_name,
                    ffi_free,
                    constructors: self.lower_constructors(&class_def.id, class_def),
                    methods: self.lower_methods(&class_def.id, class_def),
                }
            })
            .collect()
    }

    fn lower_constructors(&self, class_id: &ClassId, class_def: &ClassDef) -> Vec<RubyConstructor> {
        class_def
            .constructors
            .iter()
            // Constructors that take a composite (record / Vec / Option) or
            // class-handle argument need channels that aren't supported yet;
            // skip them rather than emit glue that can't encode the argument.
            // Fallible handle-returning ctors (try_new -> Result<Self>) use the
            // null+last_error channel and are kept.
            .filter(|ctor_def| {
                ctor_def
                    .params()
                    .iter()
                    .all(|p| Self::is_supported_param(&p.type_expr))
            })
            .map(|ctor_def| {
                let ruby_name = match ctor_def {
                    ConstructorDef::Default { .. } => "new".to_string(),
                    ConstructorDef::NamedFactory { name, .. } => naming::method_name(name.as_str()),
                    ConstructorDef::NamedInit { name, .. } => naming::method_name(name.as_str()),
                };

                let ffi_new = match ctor_def {
                    ConstructorDef::Default { .. } => class_ffi_new(class_id.as_str()).to_string(),
                    ConstructorDef::NamedFactory { name, .. } => {
                        method_ffi_name(class_id.as_str(), name.as_str()).to_string()
                    }
                    ConstructorDef::NamedInit { name, .. } => {
                        method_ffi_name(class_id.as_str(), name.as_str()).to_string()
                    }
                };

                let is_fallible = ctor_def.is_fallible();
                let params = ctor_def
                    .params()
                    .into_iter()
                    .map(|p| RubyParam {
                        ruby_name: naming::method_name(p.name.as_str()),
                        ruby_type: self.lower_type(&p.type_expr),
                    })
                    .collect();

                RubyConstructor {
                    ruby_name,
                    params,
                    ffi_new,
                    is_fallible,
                }
            })
            .collect()
    }

    fn lower_methods(&self, class_id: &ClassId, class_def: &ClassDef) -> Vec<RubyMethod> {
        let mut methods = Vec::new();
        for method_def in &class_def.methods {
            // `&mut self` cannot be safely exposed to Ruby: the receiver is a
            // GC-managed, freely-aliased handle, so Rust's exclusive-borrow
            // guarantee can't hold. Skip the method (rather than emitting
            // unsound glue or aborting the whole crate's generation) and warn
            // loudly so the author can switch to `&self` + interior mutability.
            // (Checked before the skip filters below so the author always
            // learns the real reason, even for an async/composite `&mut self`
            // method.)
            if method_def.receiver == Receiver::RefMutSelf {
                eprintln!(
                    "warning: skipping `{class}::{method}` in the Ruby bindings: it takes \
                     `&mut self`, but Ruby manages the receiver as a garbage-collected handle \
                     that is freely aliased and shared across threads/Ractors, so Rust's \
                     exclusive-borrow guarantee cannot hold. Change the receiver to `&self` and \
                     move the mutable state behind interior mutability (e.g. `RefCell<T>`).",
                    class = class_id.as_str(),
                    method = method_def.id.as_str(),
                );
                continue;
            }

            // Async methods and methods that marshal composite types (record /
            // Vec / Option returns or params, and any Result — the WireEncoded
            // channel) are not yet supported; skip them rather than emit glue
            // that can't encode the value.
            if method_def.is_async() {
                continue;
            }
            if !method_def
                .params
                .iter()
                .all(|p| Self::is_supported_param(&p.type_expr))
            {
                continue;
            }
            let Some(return_kind) = self.lower_return_kind(&method_def.returns) else {
                continue;
            };

            let ruby_name = naming::method_name(method_def.id.as_str());
            let ffi_sym = method_ffi_name(class_id.as_str(), method_def.id.as_str()).to_string();
            let is_fallible = matches!(method_def.returns, ReturnDef::Result { .. });
            let consumes_self = method_def.receiver == Receiver::OwnedSelf;
            // `Receiver::Static` methods (associated fns with no `self`) bind as
            // Ruby singleton methods: no TypedData receiver, no handle threaded
            // into the FFI call, registered with `rb_define_singleton_method`.
            let is_static = method_def.receiver == Receiver::Static;

            let params = method_def
                .params
                .iter()
                .map(|p| RubyParam {
                    ruby_name: naming::method_name(p.name.as_str()),
                    ruby_type: self.lower_type(&p.type_expr),
                })
                .collect();

            methods.push(RubyMethod {
                ruby_name,
                params,
                return_kind,
                ffi_sym,
                consumes_self,
                is_fallible,
                is_static,
            });
        }
        methods
    }

    /// Map a method's return to a supported `RubyReturnKind`, or `None` for
    /// composite/Result returns that need the WireEncoded channel.
    fn lower_return_kind(&self, returns: &ReturnDef) -> Option<RubyReturnKind> {
        match returns {
            ReturnDef::Void => Some(RubyReturnKind::Void),
            // `Vec<u8>` is the canonical byte-buffer shape (the dedicated `Bytes`
            // IR node was removed in #509); surface it as a Ruby binary string.
            ReturnDef::Value(ty) if Self::is_bytes(ty) => Some(RubyReturnKind::Bytes),
            ReturnDef::Value(ty) => match ty {
                TypeExpr::Handle(handle_class_id) => Some(RubyReturnKind::Handle(
                    naming::method_name(handle_class_id.as_str()),
                )),
                TypeExpr::String => Some(RubyReturnKind::Utf8String),
                _ if Self::is_composite(ty) => None,
                _ => Some(RubyReturnKind::Scalar(self.lower_type(ty))),
            },
            // Result returns are wire-encoded (value-returning) or use the
            // null+last_error channel (handle-returning); neither is supported yet.
            ReturnDef::Result { .. } => None,
        }
    }

    /// `Vec<u8>` — the canonical byte-buffer representation after the dedicated
    /// `TypeExpr::Bytes` node was removed (#509). Treated as a Ruby binary
    /// `String`, not an `Array<Integer>`, and crosses the ABI as a span rather
    /// than a wire-encoded value.
    fn is_bytes(ty: &TypeExpr) -> bool {
        matches!(
            ty,
            TypeExpr::Vec(inner)
                if matches!(inner.as_ref(), TypeExpr::Primitive(PrimitiveType::U8))
        )
    }

    fn is_composite(ty: &TypeExpr) -> bool {
        // `Vec<u8>` is byte data, not a composite collection.
        !Self::is_bytes(ty)
            && matches!(
                ty,
                TypeExpr::Record(_) | TypeExpr::Vec(_) | TypeExpr::Option(_)
            )
    }

    /// Param shapes the Ruby glue can marshal today: scalars, bool, enums,
    /// strings, and byte buffers. Composite (record / `Vec` / `Option`) and
    /// class-handle params need channels that aren't implemented yet, so
    /// callables using them are skipped rather than emitted with a signature
    /// that doesn't match the exported ABI.
    fn is_supported_param(ty: &TypeExpr) -> bool {
        !Self::is_composite(ty) && !matches!(ty, TypeExpr::Handle(_))
    }

    /// Free-function returns the function template can emit: void, strings,
    /// byte buffers, and scalars (incl. bool/enum). Handle returns are only
    /// supported on the class-method path; composite and `Result` returns need
    /// the WireEncoded / last_error channels.
    fn is_supported_function_return(returns: &ReturnDef) -> bool {
        match returns {
            ReturnDef::Void => true,
            ReturnDef::Value(ty) => {
                Self::is_bytes(ty)
                    || matches!(ty, TypeExpr::String)
                    || (!Self::is_composite(ty) && !matches!(ty, TypeExpr::Handle(_)))
            }
            ReturnDef::Result { .. } => false,
        }
    }

    fn lower_type(&self, ty: &TypeExpr) -> RubyType {
        if Self::is_bytes(ty) {
            return RubyType::Bytes;
        }
        match ty {
            TypeExpr::Void => RubyType::Void,
            TypeExpr::Primitive(PrimitiveType::Bool) => RubyType::Bool,
            TypeExpr::Primitive(p) => {
                if p.is_float() {
                    RubyType::Float {
                        bits: (p.size_bytes().unwrap_or(8) * 8) as u8,
                    }
                } else {
                    RubyType::Integer {
                        signed: p.is_signed(),
                        bits: (p.size_bytes().unwrap_or(8) * 8) as u8,
                    }
                }
            }
            TypeExpr::String => RubyType::RubyString,
            TypeExpr::Vec(inner) => RubyType::Array(Box::new(self.lower_type(inner))),
            TypeExpr::Record(rec_id) => RubyType::Record(naming::class_name(rec_id.as_str())),
            TypeExpr::Enum(enum_id) => RubyType::Enum(naming::class_name(enum_id.as_str())),
            TypeExpr::Handle(class_id) => RubyType::Handle(naming::method_name(class_id.as_str())),
            // Async/stream/composite types are not yet representable here.
            _ => RubyType::Void,
        }
    }
}
