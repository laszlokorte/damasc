# JEPL

Experimental REPL for a subset of an ES6 like language.

## Features

Includes only: null, boolean, string, integer, array and object literals.

The only operations that are allowed are: 
* arithmetic (`*`,`/`,`+`,`-`,`^`) on intengers, eg `3+5*7` evaluates to `38`
* comparison (`<`,`>`,`<=`,`>=`) on intengers, eg `108 > 23` evaluates to `true`
* logical operations on bools (`!`, `&&`, `||`), eg `23 > 5 && !(23 > 10)` evaluates to `false`
* strict (in)equality (`==`, `!=`), eg `[1,2,3] == [1,2,3]` evaluates to `true`, but `5 == "foo"` evaluates to `false`
* intenger-indexed access on arrays and strings (negativ index points from the end), eg `["a","b","c"][0] == ["a","b","c"][-2]`
* string-indexed access on objects, eg `"ciao"[0] == "ciao"[-4]`
* literal array construction: `[23,"foo",true]`
* literal object construction: `{foo: 42, ["bar"]: 23}`
* literal object construction with computed key: `{foo: 42, [["bar","baz"][1]]: 23} == {baz: 23, foo: 42, }`
* array spreading: `[23,24, ...[50,51]] == [23, 24, 50, 51]`
* object spreading: `{foo: 42, ...{x:23, y:16}} == {foo: 42, x: 23, y: 16, }`
* check if object key exists: `"foo" in {foo: 24}` evaluates to `true`
* in the repl variales can be stored: eg. type `x = 7` hit enter, and then later `x*x` evaluates to `49`

## Todo

* implement pattern matching
* implement simple template literal strings (for string concatination)
* implement advanced integer syntax (eg 10_000, maybe hex, oct and dual basis)
* improve error reporting

The goal is to build a simple expression based language that is not turing complete but allows for simple pattern matching and data manipulation.
It is made to be embeded as a sub language into other more powerful systems.

## Usage

Use cargo to start the REPL:

```sh
$ cargo run
```
Inside the REPL you can type simple expressions and hit <kbd>enter</kbd> to evaluate. For example:

```
>> 5+5
10
```

Take a look into the [tests.txt](./src/tests.txt) file for a few example expressions.

You can also store the result of an expression in a named variable for later use:

```
>> x := 5+5
10
>> x*x
100
```

Or print the syntax tree of an expression:

```
>> .inspect 5+5
Binary(
    BinaryExpression {
        operator: Plus,
        left: Literal(
            Number(
                "5",
            ),
        ),
        right: Literal(
            Number(
                "5",
            ),
        ),
    },
)
```