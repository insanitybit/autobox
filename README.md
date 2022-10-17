# autobox

A set of tools and libraries for automatically generating and initiating sandboxes for Rust programs

The intent is to be able to one day add a few macros to your code to label your
functions' effects and `autobox-cli` will generate a sandbox for your binary to
run in.

!!!! Note that this is currently a _proof of concept_. `autobox-cli` is even
hardcoded to only analyze the `example-app` in this repo.

This document will describe the _intent_ of the project, not necessarily what
is or is not implemented today. See the [Roadmap](#Roadmap) section for current state.

#### Example

Note that the macro and autobox "language" are unstable and likely to change.

Here's a very brief example of what using `autobox` should look like:

```
#[effect::declare(
    args=(_ as F, _ as B),
    side_effects=(
        eval(F + '/' + B) as T,
        read_that_file(T) as O
    ),
    returns=(O),
)]
fn reads_file(directory: &str, filename: &str) -> String {
    let path = format!("{}/{}", directory, filename);
    std::fs::read_to_string(path).unwrap()
}

#[effect::entrypoint]
fn main() {
    reads_that_file("~", "config.json");
}
```

`autobox` will analyse your Rust program to understand which side effects
can occur and what the arguments to those side effects are. That information
can then be fed into a sandbox generating system. In this case, one could
produce a sandbox that only allows this program to read `~/config.json`
at runtime.

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

#### The `declare` macro

The `declare` macro allows you to define inputs, outputs, and
side effects to your program. The inputs and outputs will be traced
during analysis by `autobox-cli`, allowing for sandbox generation to
create fine-grained policies.

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

#### The `infer` macro

The `infer` macro will analyze a function to determine its side effects.

```rust
#[effect::infer]
fn loadconfigs() -> (String, String) {
    let config_a = read_file("~", "config.json");
    let config_b = read_file("~", ".env");
    (config_a, config_b)
}
```

In this case `autobox-cli` will understand that `read_file` implies some
side effects and therefor would infer that `loadconfigs` would have the
side effects `read_file("~/config.json")` and `read_file("~/.env")`.

In theory `autobox` could infer information for every function in your
project, but it's inclear if that's desirable. See [Limitations](#Limitations).

#### The `entrypoint` macro

Like `infer`, `entrypoint` analyzes a function to find out its side effects.
The difference is that whereas `infer`'s analysis is lazy, `entrypoint` is where
analysis actually begins.

In most cases `entrypoint` should be applied to `main` or something similar.

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

#### Mutation

Mutating inputs to a function is represented differently from other side effects.
This is because Rust already manages mutation for you, and sandboxes don't apply
to internal memory like that.

Here's an example of what mutation looks like:

```rust
#[effect::declare(
    args=(first_path as P0, second_path as P1),
    update=(first_path becomes P0 + P1)
)]
fn append_path(first_path: &mut PathBuf, second_path: Path) {
    first_path.push(second_path);
}
```

In this example we have a function `append_path` that mutates
its first argument `first_path` by pushing `second_path` onto it.

We represent these mutations via the `update` clause, indicating that,
after `append_path` is called the state of `first_path` will be equivalent
to the evaluation of `P0 + P1`.

### `autobox-cli`

(note: None of the autobox-cli is really implemented as a cli. It is hardcoded
to only run against the example-app, and all it does is run the analysis and
print out the side effects)

#### `analyze`

`autobox-cli analyze <project-name>`

The `analyze` subcommand of the cli will execute over a project, run inference
on the entrypoint, and output that analysis.

### Limitations

Note that quite a lot of this is not implemented.

1. No implementation for mutation

2. No implementation for `infer` or `declare_ext!` macros

3. No implementation for side effects that produce a value

4. We don't trace values and instead _only_ support inline literal values,
   ex: `foo(some_var)` is not supported, only `foo("path")`.

5. Support for branching is not supported.

6. Operations other than `+` are not supported

7. The implementation is extremely hacky and brittle

8. No sandbox implementation

9. No implementation for methods or structs

10. There is no analysis of functions that are not marked with a macro. This
    means that if

- `entrypoint` calls B
- B calls C
- B is not annotated
  `entrypoint` will not infer any of `C`'s side effects

11. Function names are treated globally. As in, `my_crate::foo` and `other_crate::foo`
    can not be differentiated.

And More! See the [issue tracker](https://github.com/insanitybit/autobox).

### Open Questions

Here are some questions I haven't really nailed down good answers to.

#### What should this language _be_?

The `effect` language is not "designed". I wrote something that made
sense in my mind and then implemented the most basic evaluation phase
based on that. It has no types, it has no loops. Is that good? Should
it have those things?

#### How should new capabilities be defined?

Right now capabilities are just labels with arguments. What the
type of those arguments are, what type a capability may return, is
no defined. What would the UX look like to add a new capability?

Capabilities could always be arbitrary, and it could be up to the
sandbox generation framework to interpret them.

For now it's clear that some capabilities are definitely necessary:

1. We need file capabilities. At minimum read and write, possibly
   mmap and append.

2. We need network capabilities. Unclear what layer that should be
   at.

#### What operations should be supported? And how?

Currently I've got support for `+`, it's unclear what other operations
should be supported. Further, `+` is just this generic "add" construct,
it doesn't _mean_ anything.

Does `+` work on paths, strings, collections?

#### How should analysis execute?

It seems to me that `autobox` macros are essentially a DSL for
tracing information. As it's a DSL, I wonder if it would make sense
to just use a _real_ programming language under the hood. For example,
could we just compile all of this into something like prolog/datalog and
have it do the execution?

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
2. Implement an actual parser for the `effect` language
3. Implement tracing of variables for `entrypoint`
4. Implement `infer`
5. Generation of a sandbox, probably seccomp to start with
