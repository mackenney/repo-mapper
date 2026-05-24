; Definitions
(class_declaration name: (identifier) @name.definition.class)
(function_declaration name: (identifier) @name.definition.function)
(generator_function_declaration name: (identifier) @name.definition.function)

; Method definitions (exclude constructor)
(method_definition
  name: (property_identifier) @name.definition.method
  (#not-eq? @name.definition.method "constructor"))

; Variable/const function assignments
(lexical_declaration
  (variable_declarator
    name: (identifier) @name.definition.function
    value: [(arrow_function) (function_expression)]))

; Object property function assignments
(pair
  key: (property_identifier) @name.definition.function
  value: [(arrow_function) (function_expression)])

; References - calls (exclude require)
(call_expression
  function: (identifier) @name.reference.call
  (#not-eq? @name.reference.call "require"))
(call_expression
  function: (member_expression property: (property_identifier) @name.reference.call))

; new expressions
(new_expression constructor: (identifier) @name.reference.class)
