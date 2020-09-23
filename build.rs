use clap::Shell;

include!("src/cli.rs");

fn main() {
    let outdir = match std::env::var_os("OUT_DIR") {
        None => return,
        Some(o) => o,
    };

    build_cli().gen_completions("emlop", Shell::Bash, outdir);
}
