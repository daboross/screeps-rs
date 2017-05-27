extern crate screeps_rs;
extern crate clap;

use clap::{Arg, App};

fn main() {
    let matches = App::new("screeps-rs")
        .version("0.0.1")
        .author("David Ross <daboross@daboross.net>")
        .about("Native client for the Screeps JavaScript MMO")
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .multiple(true)
            .help("enables verbose logging"))
        .get_matches();

    screeps_rs::main(matches.is_present("verbose"));
}
