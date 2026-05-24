; Definitions
(class_definition name: (identifier) @name.definition.class)
(function_definition name: (identifier) @name.definition.function)

; Module-level assignments (constants)
(module
  (expression_statement
    (assignment
      left: (identifier) @name.definition.constant)))

; References - call targets
(call function: (identifier) @name.reference.call)
(call function: (attribute attribute: (identifier) @name.reference.call))
