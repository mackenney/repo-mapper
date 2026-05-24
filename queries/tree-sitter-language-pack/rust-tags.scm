; Definitions
(struct_item name: (type_identifier) @name.definition.class)
(enum_item name: (type_identifier) @name.definition.class)
(union_item name: (type_identifier) @name.definition.class)
(type_item name: (type_identifier) @name.definition.class)
(trait_item name: (type_identifier) @name.definition.interface)
(function_item name: (identifier) @name.definition.function)
(mod_item name: (identifier) @name.definition.module)
(macro_definition name: (identifier) @name.definition.macro)
(impl_item type: (type_identifier) @name.definition.class)

; Methods in impl blocks
(impl_item
  body: (declaration_list
    (function_item name: (identifier) @name.definition.method)))

; References
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (field_expression field: (field_identifier) @name.reference.call))
(macro_invocation macro: (identifier) @name.reference.call)

; Trait/type references in impl
(impl_item trait: (type_identifier) @name.reference.implementation)
