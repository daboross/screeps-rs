extern crate conrod_testing;
extern crate clap;

use clap::{Arg, App};

fn main() {
    let matches = App::new("screeps-conrod-testbed")
        .version("0.0.1")
        .author("David Ross <daboross@daboross.net>")
        .about("Native client for the Screeps JavaScript MMO")
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .multiple(true)
            .help(""))
        .get_matches();

    conrod_testing::main(matches.is_present("verbose"));
}
