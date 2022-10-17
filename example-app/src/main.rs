use autobox_effect as effect;

// This is an example of some of what can be done today
#[effect::declare(
    args=(_ as F, _ as B),
    side_effects=(
        eval(F + '/' + B) as T,
        read_file(T) as O
    ),
    // unimplemented
    returns=(O),
)]
fn reads_file(directory: &str, filename: &str) -> String {
    let path = format!("{}/{}", directory, filename);
    std::fs::read_to_string(path).unwrap()
}

#[effect::declare(
    args=(_ as D, _ as F),
    returns=(eval(D + '/' + F)),
)]
fn join_path(a: &str, b: &str) -> String {
    format!("{a}/{b}")
}

// By inspecting `entrypoint` we can *infer* capabilities based on the functions called
// within the function.

#[effect::entrypoint]
fn main() {
    // This creates a new variable that we understand as "~/dir/"
    let file_dir = join_path("~", "dir/");
    // This generates a side effect. Currently `file_dir` will
    // effectively become a glob, "*", and become "read_file(*/example.txt)"
    // In the future we'll be able to trace variables and their constraints,
    // allowing us to generate an effect on `"~/dir/example.txt"` precisely
    let _file_contents = reads_file(&file_dir, "example.txt");
}
