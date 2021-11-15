# Simple expression language

Yuck includes a small expression language that can be used to run several operations on your data.
This can be used to show different values depending on certain conditions,
do mathematic operations, and even access values withing JSON-structures.

These expressions can be placed anywhere within your configuration inside `{ ... }`,
as well as within strings, inside string-interpolation blocks (`"foo ${ ... } bar"`).

## Example

```lisp
(box
  "Some math: ${12 + foo * 10}"
  (button :class {button_active ? "active" : "inactive"}
          :onclick "toggle_thing"
    {button_active ? "disable" : "enable"}))
```

## Features

Supported currently are the following features:
- simple mathematical operations (`+`, `-`, `*`, `/`, `%`)
- comparisons (`==`, `!=`, `>`, `<`)
- boolean operations (`||`, `&&`, `!`)
- elvis operator (`?:`)
    - if the left side is `""`, then returns the right side, otherwise evaluates to the left side.
- conditionals (`condition ? 'value' : 'other value'`)
- numbers, strings, booleans and variable references (`12`, `'hi'`, `true`, `some_variable`)
- json access (`object.field`, `array[12]`, `object["field"]`)
    - for this, the object/array value needs to refer to a variable that contains a valid json string.
- some function calls:
    - `round(number, decimal_digits)`: Round a number to the given amount of decimals
    - `replace(string, regex, replacement)`: Replace matches of a given regex in a string

