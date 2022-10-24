# autobox

_Compile time analysis for runtime sandboxing_

The intent is to be able to one day add a few macros to your code to label your
functions' effects and `autobox-cli` will generate a sandbox for your binary to
run in.

!!!! Note that this is currently a _proof of concept_. `autobox-cli` is even
hardcoded to only analyze the `example-app` in this repo.

This document will describe the _intent_ of the project, not necessarily what
is or is not implemented today. See the [Roadmap](#Roadmap) section for current state.

#### Example

Note that the macro and autobox "language" are unstable and likely to change.

`autobox` should primarily be powered by *inference*. That is, you shouldn't
have to write many annotations, autobox will use what it has to figure things
out.

```rust
#[effect::declare(
    args=(a as A, b as B),
    side_effects=(reads_file(A + '/' + B)),
    returns=(A + '/' + B),
)]
fn fn_with_effects(a: &str, b: &str) -> String {
    std::fs::read_to_string(format!("{a}/{b}"));
    format!("{a}/{b}")
}

fn unknown(a: &str, b: &str) -> String {
    let c = fn_with_effects(a, b);
    fn_with_effects(&c, "config_file.json")
}

#[effect::entrypoint]
fn main() {
    let x = "~";
    let y = x;
    let _uk = unknown(y, "config_dir");
}
```

If you were to run `autobox-cli` against this program it would output:
```
Side effect: reads_file("~/config_dir")
Side effect: reads_file("~/config_dir/config_file.json")
```

The analysis tells us that the `entrypoint` target will perform a `reads_file`
operation parameterized by one of two strings. Those strings were inferred by
tracing their initial values through the code.

Using this information, one could generate a policy to constrain their program
at runtime using a technology such as seccomp or apparmor. 

`autobox` is comprised of:

1. The `autobox-effect` macro crate (`effect`)
2. The `autobox-cli` app (`autobox`)

#### What is a side effect?

Side effects are things that your function does that have impact outside
of that function's scope. That could mean mutating a value through `&mut`,
performing I/O, or anything observable from outside of that function's scope.

In `autobox` side-effects are represented as labels that take arguments and produce
an output. For example, `read_file("*config.json")`.

The label `read_file` is arbitrary, as are its arguments. In the future there
should be a clearer path to defining these functions and how they should
behave.

#### What is a sandbox?

Sandboxes are runtime limitations on what your program can do. While Rust
provides _compile time_ verification of how your program _should_ behave,
a sandbox will force how your program _can_ behave - even if an attacker
has full control over that program's execution.

Sandboxing technologies work in many different ways. The `seccomp` sandbox
is a Linux technology that limits which system calls can be made by a program
and what the arguments can be. `Apparmor` is a Linux Security Model that
constrains a process in terms of file, network, and other accesses based on
a profile.

### The Components of `autobox`

### `autobox-effect`

The `autobox-effect` crate exports macros used by the `autobox-cli` analysis
to understand the "side effects" that your program has. For example,

#### The `entrypoint` macro
The `entrypoint` macro determines where analysis of a program's side effects
will begin. This will usually be 'main'.

#### The `declare` macro

The `declare` macro allows you to define inputs, outputs, and
side effects to a function. No inference is performed against
functions marked with `declare`, `declare` is an *unchecked assertion*
about what the function does. If you forget a side effect in `declare`
or otherwise improperly declare your function you will potentially run
into improper analysis results.


```rust
use autobox_effect as effect;

#[effect::declare(
    args=(_ as F, _ as B),
    side_effects=(
        eval(F + '/' + B) as T,
        read_file(T) as O
    ),
    returns=(O),
)]
fn reads_that_file(directory: &str, filename: &str) -> String {
    let path = format!("{}/{}", directory, filename);
    std::fs::read_to_string(path).unwrap()
}
```
#### The `declare_ext!` macro

In some cases it will be necessary to declare the effects of external
functions that you can't apply a macro to directly. One example of this would
be the standard library, which has tons of functions we'd love to have declare
their side effects.

For this, we have the `declare_ext!` macro.

```rust
declare_ext!(
    std::file::File::create,
    args=(path as P),
    side_effects=(write_file(P) as F),
    returns=(F),
);
```

In this instance we are declaring that the function at path
`std::file::File::create` takes in an argument as `P`, has the side
effect `write_file(P)`, which produces a value `F`, and returns `F`.


### `autobox-cli`

(note: Note that currently `autobox-cli` is hardcoded
to only run against the example-app, and all it does is run the analysis and
print out the side effects. The below represents some work that has not yet
been completed)

#### `analyze`

`autobox-cli analyze <project-name>`

The `analyze` subcommand of the cli will execute over a project, run inference
on the entrypoint, and output that analysis.

#### `generate`
A theoretical command that would take the output of `analyze` and feed it to
a sandbox policy generator

`autobox-cli generate --policy-engine=apparmor`


### Limitations

Note that quite a lot of this is not implemented.

1. No implementation for mutation

2. No implementation for `declare_ext!` macros

3. No implementation for side effects that produce a value

4. Support for branching is not supported.

5. Operations other than `+` are not supported

6. No sandbox implementation

7. No implementation for methods or structs

8. Function names are treated globally. As in, `my_crate::foo` and `other_crate::foo`
    can not be differentiated.

And More! See the [issue tracker](https://github.com/insanitybit/autobox).

### Open Questions

Here are some questions I haven't really nailed down good answers to.

#### How will generics be handled?

It's unclear how to handle generics, or where that will come to play. For
example, if I have a function like this, what are its effects?

```rust
fn reads_a_thing(r: impl Read) {
    r.read(&mut vec![]);
}
```

If `r` is just a `Cursor<Vec<u8>>` there is no effect. If it's a file there is.
Things get worse with `dyn` I imagine.

### Roadmap

This POC is exactly that, a _POC_. Before I continue adding anything to
the implementation I believe there must be some work to better define
the system. When that is done I'm sure some aspects of the roadmap will
change, but _generally speaking_, this is what I'd like to do:

1. Better define and document the `effect` language
2. ~Implement an actual parser for the `effect` language~
3. ~Implement tracing of variables for `entrypoint`~
4. ~Implement `infer`~
5. Generation of a sandbox, probably seccomp to start with
