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
- elvis operator (`?:`)
    - if the left side is `""`, then returns the right side, otherwise evaluates to the left side.
- conditionals (`if condition then 'value' else 'other value'`)
- numbers, strings, booleans and variable references (`12`, `'hi'`, `true`, `some_variable`)
    - strings can contain other expressions again: `'foo {{some_variable}} bar'`
- json access (`object.field`, `array[12]`, `object["field"]`)
    - for this, the object/array value needs to refer to a variable that contains a valid json string.
- some function calls:
    - `round(number, decimal_digits)`: Round a number to the given amount of decimals
    - `replace(string, regex, replacement)`: Replace matches of a given regex in a string

