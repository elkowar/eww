+++
title = "Eww expressions"
slug = "Embedded eww expression language"
weight = 6
+++

# The embedded Eww expression-language

Within variable references, you can make use of a small, built-in expression language.
This can be used whereever you can use variable-references (`{{varname}}`).

## Example

```xml
<button
    class="{{if button_active then 'active' else 'inactive'}}"
    onclick="toggle_thing">
    {{if button_active then 'disable' else 'enable'}}
</button>

Some math: {{12 + 2 * 10}}
```

## Syntax

The expression language supports:
- simple mathematical operations (`+`, `-`, `*`, `/`, `%`)
- comparisons (`==`, `!=`, `>`, `<`)
- boolean operations (`||`, `&&`, `!`)
- conditionals (`if condition then 'value' else 'other value'`)
- numbers, strings, booleans and variable references (`12`, `'hi'`, `true`, `some_variable`)
    - strings can contain other expressions again: `'foo {{some_variable}} bar'`

