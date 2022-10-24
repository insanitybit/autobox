use autobox_effect as effect;


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
    let _uk = unknown("~", "config_dir");
}