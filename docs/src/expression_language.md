# Simple expression language

Yuck includes a small expression language that can be used to run several operations on your data.
This can be used to show different values depending on certain conditions,
do mathematic operations, and even access values within JSON-structures.

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
- regex match operator (`=~`)
    - Rust regex style, left hand is regex, right hand is string
    - ex: workspace.name =~ '^special:.+$'
- elvis operator (`?:`)
    - if the left side is `""` or a JSON `null`, then returns the right side,
      otherwise evaluates to the left side.
- Safe Access operator (`?.`) or (`?.[index]`)
    - if the left side is an empty string or a JSON `null`, then return `null`. Otherwise,
      attempt to index. Note that indexing an empty JSON string (`'""'`) is an error.
    - This can still cause an error to occur if the left hand side exists but is
      not an object or an array.
      (`Number` or `String`).
- conditionals (`condition ? 'value' : 'other value'`)
- numbers, strings, booleans and variable references (`12`, `'hi'`, `true`, `some_variable`)
- json access (`object.field`, `array[12]`, `object["field"]`)
    - for this, the object/array value needs to refer to a variable that contains a valid json string.
- some function calls:
    - `round(number, decimal_digits)`: Round a number to the given amount of decimals
    - `floor(number)`: Round a number down to the nearest integer
    - `ceil(number)`: Round a number up to the nearest integer
    - `sin(number)`, `cos(number)`, `tan(number)`, `cot(number)`: Calculate the trigonometric value of a given number in **radians**
    - `min(a, b)`, `max(a, b)`: Get the smaller or bigger number out of two given numbers
    - `powi(num, n)`, `powf(num, n)`: Raise number `num` to power `n`. `powi` expects `n` to be of type `i32`
    - `log(num, n)`: Calculate the base `n` logarithm of `num`. `num`, `n` and return type are `f64`
    - `degtorad(number)`: Converts a number from degrees to radians
    - `radtodeg(number)`: Converts a number from radians to degrees
    - `replace(string, regex, replacement)`: Replace matches of a given regex in a string
  - `search(string, regex)`: Search for a given regex in a string (returns array)
  - `matches(string, regex)`: check if a given string matches a given regex (returns bool)
  - `captures(string, regex)`: Get the captures of a given regex in a string (returns array)
  - `strlength(value)`: Gets the length of the string
    - `substring(string, start, length)`: Return a substring of given length starting at the given index
  - `arraylength(value)`: Gets the length of the array
  - `objectlength(value)`: Gets the amount of entries in the object
  - `jq(value, jq_filter_string)`: run a [jq](https://jqlang.github.io/jq/manual/) style command on a json value. (Uses [jaq](https://crates.io/crates/jaq) internally).
  - `jq(value, jq_filter_string, args)`: Emulate command line flags for jq, see [the docs](https://jqlang.github.io/jq/manual/#invoking-jq) on invoking jq for details. Invalid flags are silently ignored.
    Currently supported flags:
    - `"r"`: If the result is a string, it won't be formatted as a JSON string. The equivalent jq flag is `--raw-output`.
  - `get_env(string)`: Gets the specified enviroment variable
  - `formattime(unix_timestamp, format_str, timezone)`: Gets the time in a given format from UNIX timestamp.
     Check [chrono's documentation](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) for more
     information about format string and [chrono-tz's documentation](https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html)
     for available time zones.
  - `formattime(unix_timestamp, format_str)`: Gets the time in a given format from UNIX timestamp.
     Same as other `formattime`, but does not accept timezone. Instead, it uses system's local timezone.
     Check [chrono's documentation](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) for more
     information about format string.
