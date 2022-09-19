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
- comparisons (`==`, `!=`, `>`, `<`, `<=`, `>=`)
- boolean operations (`||`, `&&`, `!`)
- elvis operator (`?:`)
    - if the left side is `""` or a JSON `null`, then returns the right side,
      otherwise evaluates to the left side.
- Safe Access operator (`?.`)
    - if the left side is `""` or a JSON `null`, then return `null`. Otherwise,
      attempt to index.
    - This can still cause an error to occur if the left hand side exists but is
      not an object.
      (`Number` or `String`).
- conditionals (`condition ? 'value' : 'other value'`)
- numbers, strings, booleans and variable references (`12`, `'hi'`, `true`, `some_variable`)
- json access (`object.field`, `array[12]`, `object["field"]`)
    - for this, the object/array value needs to refer to a variable that contains a valid json string.
- some function calls:
    - `round(number, decimal_digits)`: Round a number to the given amount of decimals
    - `replace(string, regex, replacement)`: Replace matches of a given regex in a string
	- `search(string, regex)`: Search for a given regex in a string (returns array)
	- `matches(string, regex)`: check if a given string matches a given regex (returns bool)
	- `captures(string, regex)`: Get the captures of a given regex in a string (returns array)
	- `strlength(value)`: Gets the length of the string
	- `arraylength(value)`: Gets the length of the array
	- `objectlength(value)`: Gets the amount of entries in the object

