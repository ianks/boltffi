pub use boltffi_ffi_rules::transport::{
    DirectBufferReturnMethod, EncodedReturnStrategy, ErrorReturnStrategy, ReturnContract,
    ReturnInvocationContext, ReturnPlatform, ScalarReturnStrategy, ValueReturnMethod,
    ValueReturnStrategy,
};
use syn::{ReturnType, Type};

use crate::index::class_types::ClassTypeRegistry;
use crate::index::custom_types::CustomTypeRegistry;
use crate::index::data_types::DataTypeRegistry;
use crate::lowering::transport::NamedTypeTransportClassifier;

use super::classify::classify_value_return_strategy;

#[derive(Clone)]
pub struct ResolvedReturn {
    rust_type: syn::Type,
    return_contract: ReturnContract,
}

impl ResolvedReturn {
    pub fn new(rust_type: syn::Type, return_contract: ReturnContract) -> Self {
        Self {
            rust_type,
            return_contract,
        }
    }

    pub fn rust_type(&self) -> &syn::Type {
        &self.rust_type
    }

    pub fn value_return_strategy(&self) -> ValueReturnStrategy {
        self.return_contract.value_strategy()
    }

    pub fn encoded_return_strategy(&self) -> Option<EncodedReturnStrategy> {
        match self.return_contract.value_strategy() {
            ValueReturnStrategy::Buffer(strategy) => Some(strategy),
            _ => None,
        }
    }

    pub fn is_unit(&self) -> bool {
        matches!(
            self.return_contract.value_strategy(),
            ValueReturnStrategy::Void
        )
    }

    pub fn is_primitive_scalar(&self) -> bool {
        matches!(
            self.return_contract.value_strategy(),
            ValueReturnStrategy::Scalar(ScalarReturnStrategy::PrimitiveValue)
        )
    }

    pub fn is_passable_value(&self) -> bool {
        matches!(
            self.return_contract.value_strategy(),
            ValueReturnStrategy::Scalar(ScalarReturnStrategy::CStyleEnumTag)
                | ValueReturnStrategy::CompositeValue
        )
    }

    pub fn is_object_handle(&self) -> bool {
        matches!(
            self.return_contract.value_strategy(),
            ValueReturnStrategy::ObjectHandle
        )
    }

    pub fn value_return_method(
        &self,
        context: ReturnInvocationContext,
        platform: ReturnPlatform,
    ) -> ValueReturnMethod {
        self.return_contract.value_return_method(context, platform)
    }

    pub fn direct_buffer_return_method(
        &self,
        context: ReturnInvocationContext,
        platform: ReturnPlatform,
    ) -> Option<DirectBufferReturnMethod> {
        self.return_contract
            .direct_buffer_return_method(context, platform)
    }
}

#[derive(Clone, Copy)]
pub struct ReturnLoweringContext<'a> {
    custom_types: &'a CustomTypeRegistry,
    data_types: &'a DataTypeRegistry,
    class_types: &'a ClassTypeRegistry,
}

impl<'a> ReturnLoweringContext<'a> {
    pub fn new(
        custom_types: &'a CustomTypeRegistry,
        data_types: &'a DataTypeRegistry,
        class_types: &'a ClassTypeRegistry,
    ) -> Self {
        Self {
            custom_types,
            data_types,
            class_types,
        }
    }

    pub fn custom_types(&self) -> &'a CustomTypeRegistry {
        self.custom_types
    }

    pub fn data_types(&self) -> &'a DataTypeRegistry {
        self.data_types
    }

    pub fn class_types(&self) -> &'a ClassTypeRegistry {
        self.class_types
    }

    pub(crate) fn named_type_transport_classifier(&self) -> NamedTypeTransportClassifier<'a> {
        NamedTypeTransportClassifier::new(self.custom_types, self.data_types)
    }

    pub fn lower_output(&self, output: &ReturnType) -> ResolvedReturn {
        match output {
            ReturnType::Default => ResolvedReturn::new(
                syn::parse_quote!(()),
                ReturnContract::infallible(ValueReturnStrategy::Void),
            ),
            ReturnType::Type(_, rust_type) => self.lower_type(rust_type),
        }
    }

    pub fn lower_type(&self, rust_type: &Type) -> ResolvedReturn {
        ResolvedReturn::new(
            rust_type.clone(),
            ReturnContract::new(
                classify_value_return_strategy(rust_type, self),
                ErrorReturnStrategy::None,
            ),
        )
    }
}

#[derive(Clone, Copy)]
pub struct WasmOptionScalarEncoding {
    pub(super) primitive: boltffi_ffi_rules::primitive::Primitive,
}
