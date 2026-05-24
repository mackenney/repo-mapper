; Definitions
(class_declaration name: (type_identifier) @name.definition.class)
(abstract_class_declaration name: (type_identifier) @name.definition.class)
(interface_declaration name: (type_identifier) @name.definition.interface)
(type_alias_declaration name: (type_identifier) @name.definition.type)
(enum_declaration name: (identifier) @name.definition.enum)
(module name: (identifier) @name.definition.module)
(function_declaration name: (identifier) @name.definition.function)
(function_signature name: (identifier) @name.definition.function)

; Method definitions
(method_definition name: (property_identifier) @name.definition.method)
(method_signature name: (property_identifier) @name.definition.method)
(abstract_method_signature name: (property_identifier) @name.definition.method)

; References - type annotations
(type_annotation (type_identifier) @name.reference.type)
(type_arguments (type_identifier) @name.reference.type)

; new expressions
(new_expression constructor: (identifier) @name.reference.class)
