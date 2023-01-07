# Damasc

Experimental language for a subset of an ES6 inspired language. The feature set focuses on object and array literals, destructuring and pattern matching. Supports only immutable value types semantics, no reference types.

The syntax and semantic are not a real subset of ES6. The name is a reference to the patterns of [Damascus steel](https://en.wikipedia.org/wiki/Damascus_steel).

## Features

Includes only: null, boolean, string, integer, array and object literals. 

The only operations that are allowed are: 
* the type of a value can be checked via `is` operator. eg `(5*3) is Integer` evaluates to `true`.
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
* the `length` function tells the size of a string, array or object. eg `length([1,2,3])` evaluate to `3`
* the `type` function tells the values type: `type("Hello") == String`
* The types are accessible as literals: `Boolean`, `Integer`, `String`, `Null`, `Object`, `Array`, `Type`. Also `type(Type) == Type && type(Boolean) is Type`
* in the repl variales can be stored: eg type `let x = 7` hit enter, and then later `x*x` evaluates to `49`
* on the left side of the `=` a destructuring pattern is allowed. eg `[_,{x,...},...] = ["foo", {x:5,y:8}, true]` destructures the array on the right side and assigns the value 5 to the variable x. For more examples take a look at the [test_patterns.txt](./src/test_patterns.txt).
* when using the `let` keyword in front of a pattern the matching variables are actually assigned. Without the `let` keyword the matches displayed but then discarded.

## Not ES6

Damasc is not a replacement for ES6 but only inspired by a specific subset of features and syntax. Even regarding this subset there are differences.

* ES6 Objects and Arrays are reference types. Damasc Arrays and Objects are value types.
* ES6 allows `var [xx,xx] = [4,8]` to match and assign `x=8`. Damasc rejects the pattern because on the left hand side an array with two equal items is expected but on the right hand side an array with two different items.
* ES6 rejects `let [x,x] = [42,42]` because the variable x is declared two times in the same scope. But Damasc accepts the pattern and assigns `x=42` because the the two values corresponding to the entries in the array are equal. 
* The ES6 power operator is `**` (eg `x**2`). The Damasc power operator is `^`. Damasc currently does not support bit-wise operations.
* Damasc does not support closures, classes or any user defined functions.
* There are many more differences 

## Todo

* implement simple template literal strings (for string concatination)
* implement advanced integer syntax (eg 10_000, maybe hex, oct and dual basis)
* improve error reporting
* Allow collections (bags, multisets) of values to be created and queried via pattern syntax. eg: 
```
>> .bag mybag
OK
>> .store {x:23, y:42}
OK
>> .store {x:16, y:16}
OK
>> .count {x,y} where x > 10
2
OK
>> .query x*y <- {x,y} where x > 10
966
256
OK
>> .replace {x:y, y:x} <- {x,y} where x > 10
OK
>> .delete({x,y} where x == y)
DEL: {x:16, y:16}
```

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

Take a look into the [tests.txt](./src/test_expressions.txt) file for a few example expressions.

You can also store the result of an expression in a named variable for later use:

```
>> let x = 5+5
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

Or print the syntax tree of a pattern:

```
>> .pattern [_,_ is Boolean,{x},...]
&pattern = Array(
    [
        Pattern(
            Discard,
        ),
        Pattern(
            TypedDiscard(
                Boolean,
            ),
        ),
        Pattern(
            Object(
                [
                    Single(
                        Identifier {
                            name: "x",
                        },
                    ),
                ],
                Exact,
            ),
        ),
    ],
    Discard,
)
```

## Value Bags

You can insert values into a in memory database to be queried later:

```
>> .insert 42
OK
```

Duplicate values can be inserted:

```
>> .insert 23
OK
>> .insert 23
OK
```

You can insert multiple values at once:

```
>> .insert 108;"hello";[1,2,3]
OK
```

You can query the dataset:

```
>> .query _
42
23
23
108
"hello"
[1,2,3]
```

The number of results can be limited:

```
>> .query _ limit 2
42
23
```

The number of results can be limited:

```
>> .query x into x*x limit 2
42
23
```

The queried values can be captured in a variable and transformed:

```
>> .query x into x*x limit 2
529
1764
```

A pattern can be used to query only matching values:

```
>> .query [x,y,z]
[1,2,3]
```

The identifiers bound by the pattern can be transformed:
```
>> .query [x,y,z] into x+y*z
7
```

The results can be filtered by an additional predicate:

```
>> .query [x,y,z] into x+y*z where z > x
7
```

Pattern, transformation, filter and limit at once:

```
>> .query [x,y,z] into x+y*z where z > x limit 1
7
```

You can use multiple patterns (for now only two) to join multiple values:

```
# queries all pairs of integers a and b and transforms them into a triplet of themself and their product.
>> .query a is Integer;b is Integer into [a, b, a*b] where a > b
[42, 23, 966, ]
[42, 23, 966, ]
[108, 42, 4536, ]
[108, 23, 2484, ]
[108, 23, 2484, ]
```

You can export all inserted values into a text file (one value per line):
(currently filenames must be `[a-z_]+`)

```
>> .dump my_values
OK
```

And later read them back in:

```
>> .load my_values
OK
```

You can also delete all values currently in the memory database:

```
>> .delete _
OK
```

Or only all those matching a given pattern. For example all objects have x and y properties and whose x property equals 23:

```
>> .delete {x:23, y}
OK
```

Or deleting all Strings:

```
>> .delete _ is String
OK
```

As for the queries you can append an additional predicate to select a subset of values to be deleted. For example deleting all objects whose x property is greater than the y property:

```
>> .delete {x,y} where x>y
OK
```

You can also limit the amount of objects to delete. The following command deletes only 10 strings:

```
>> .delete _ is String limit 10
OK
```