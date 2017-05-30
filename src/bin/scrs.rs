extern crate screeps_rs;
extern crate clap;

use clap::{Arg, App};

fn main() {
    let matches =
        App::new("screeps-rs")
            .version("0.0.1")
            .author("David Ross <daboross@daboross.net>")
            .about("Native client for the Screeps JavaScript MMO")
            .arg(Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("enables verbose logging"))
            .arg(clap::Arg::with_name("debug-modules")
                .short("d")
                .long("debug")
                .value_name("MODULE_PATH")
                .help("Enable verbose logging for a specific module")
                .long_help("Enables verbose debug logging for an individual rust module or path.\nFor example, \
                            `--debug screeps_rs::app::ui` will enable verbose logging for UI related \
                            code.\n\nCommon modules you can use:\n- screeps_rs::app       app glue and UI\n- \
                            screeps_rs::app::ui   app UI\n- screeps_rs::network   app network calling and result \
                            caching\n- screeps_api           HTTP networking, websocket networking and result \
                            parsing\n- screeps_api::sockets  websocket networking only\n- hyper                 raw \
                            HTTP client\n- ws                    raw websocket client")
                .takes_value(true)
                .multiple(true))
            .get_matches();

    screeps_rs::main(matches.is_present("verbose"),
                     matches.values_of("debug-modules"));
}
