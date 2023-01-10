# Damasc

Experimental expression based language inspired by a subset of ES6. The feature set focuses on object and array literals, destructuring and pattern matching. Supports only immutable value type semantics, no reference types.

The syntax and semantic are not a strict subset of ES6. The name is a reference to the patterns of [Damascus steel](https://en.wikipedia.org/wiki/Damascus_steel).

## Live demo

Check out [damasc.laszlokorte.de](https://damasc.laszlokorte.de/) for a live demo.
The demo is compiled to WASM and runs fully locally in the browser. Currently loading and storing data from or to a file does not work.

## Features

Includes only: Null, Boolean, String, Integer, Array and Object types. 

The only operations that are allowed are: 

* the type of a value can be checked via `is` operator. eg `42 is Integer` evaluates to `true`.
* values can be casted into other types via `as` operator. eg `42 as String` evaluates to `"42"`. Not every value can be casted into every type. Only the most straight forward conversions are allowed. The specifics may change in the future.
* arithmetic (`*`,`/`,`+`,`-`,`^`) on intengers, eg `3+5*7` evaluates to `38`
* comparison (`<`,`>`,`<=`,`>=`) on intengers, eg `108 > 23` evaluates to `true`
* logical operations on bools (`!`, `&&`, `||`), eg `23 > 5 && !(23 > 10)` evaluates to `false`
* strict (in)equality (`==`, `!=`), eg `[1,2,3] == [1,2,3]` evaluates to `true`, but `5 == "foo"` evaluates to `false`, `5 == "5"` is also false.
* intenger-indexed access on arrays (negativ index points from the end), eg `["a","b","c"][0] == ["a","b","c"][-2]`
* integer-index access on strings, eg `"ciao"[0] == "ciao"[-4]`
* string-indexed access on objects, eg `{x:42,y:23}["x"] == 23`
* shorthand access on objects, eg `{x:42,y:23}.x == 23`
* string concatination via template strings, eg `` `x + y = ${x+y}` `` evaluates to `"3+7 = 10"` if `x` equals `3` and `y` equals `7`
* literal array construction: `[23, "foo", true]`
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
* Equality is always strict, no coercen: `5 != "5"`
* Damasc does not support closures, classes or any user defined functions. Support for some kind of user defined functions might be added later.
* There are many more differences.

## Todo

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

You can insert values into a in memory dataset to be queried later:

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
>> .insert 108; "hello"; [1,2,3]
OK
```

You can query the items in the dataset:

```
>> .query
42
23
23
108
"hello"
[1,2,3]
```

The number of results can be limited:

```
>> .query limit 2
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

You can join multiple values by querying multiple patterns at once. The command below queries all pairs of integers `a` and `b` from the dataset and transforms them into a triplet of each of their value and their product.

```
>> .query a is Integer;b is Integer into [a, b, a*b] where a > b
[42, 23, 966, ]
[42, 23, 966, ]
[108, 42, 4536, ]
[108, 23, 2484, ]
[108, 23, 2484, ]
```

By default the joined values are distict, i.e. in the query above a and b are always different. But you can also join across duplicates. Below we first delete all values and then only insert `1` and `0`. The first `query` results in all two permutations of `1` and `0`. Then `queryx` (notice the `x`) results in all four combinations. The third query (without `x`) tries to find all triplets of distinct values but there are none because we only inserted two values (`1` and  `0`). The last `queryx` (with x) finds all eight (`8=2^3`) possible combinations for building triples of `1` and `0`.

Currently the number of values you can join is limited 6. 

```
>> .delete _
OK
>> .insert 1;0
OK
>> .query a;b
[1, 0, ]
[0, 1, ]
>> .queryx a;b
[1, 1, ]
[1, 0, ]
[0, 1, ]
[0, 0, ]
>> .query a;b;c
>> .queryx a;b;c
[1, 1, 1, ]
[1, 1, 0, ]
[1, 0, 1, ]
[1, 0, 0, ]
[0, 1, 1, ]
[0, 1, 0, ]
[0, 0, 1, ]
[0, 0, 0, ]
```

You can export all values currently in the dataset into a text file (one value per line):
(currently for simplicty only `/[a-z_]+/` are a valid file names)

```
>> .dump my_values
OK
```

And later read them back in:

(*Caution:* The dataset is a bag/multiset. Loading values which are already in the bag will add them again)

```
>> .load my_values
OK
```

You can also delete all values currently in the dataset:

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

You can create multiple distinct bags. The initial bag is called `init`. To create a new empty bag type `.bag <somename>`:

```
>> .bag groceries
CREATED BAG
```

Values are always inserted into the current bag, deleted from the current bag und queried from the current bag. To switch to another bag use the same command as for creation:

```
>> .bag init
SWITCHED BAG
```

The `.load` and `.store` commands explained above act on the current bag as well.

To tell which bag is currently select just type `.bag`.

```
>> .bag
Current Bag: init
```

You can also create a constrained bag that accepts only values matching a given pattern. 

```
>> .bag users as {username: _ is String, age: _ is Integer}
CREATED BAG
>> .insert "Luke"
NO
>> .insert {username: "Hurley", age: 42}
INSERTED 1
```

Or a given predicate:

```
>> .bag adults as {username: _ is String, age: age is Integer} where age >= 18
CREATED BAG
>> .insert {username: "Matilda", age: 8}
NO
>> .insert {username: "Hurley", age: 42}
INSERTED 1
```

Or limit the number of items:

```
>> .bag admins as {username: _ is String} limit 1
CREATED BAG
>> .insert {username: "Locke"}
INSERTED 1
>> .insert {username: "Jack"}
NO
>> .delete _
DELETED 1
>> .insert {username: "Jack"}
INSERTED 1
```

## Build targets

Currently Damasc can be run in three different ways:

1. as Command line interface (CLI) `cargo run --bin cli`
2. as web server responding to HTTP POST requests evaluating expressions server side `cargo run --bin web --features web`
3. as static HTML/JS/WASM page running all calculations locally in a web browser. `wasm-pack build --target web --no-default-features  --out-dir ./public/wasm`, then serving `public/index.html` via local webserver for exaple `cargo server --open --path public`