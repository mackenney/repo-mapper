; Definitions
(function_declaration name: (identifier) @name.definition.function)
(method_declaration name: (field_identifier) @name.definition.method)
(type_declaration (type_spec name: (type_identifier) @name.definition.type))

; References
(call_expression function: (identifier) @name.reference.call)
(call_expression function: (selector_expression field: (field_identifier) @name.reference.call))
(type_identifier) @name.reference.type
