#[macro_use]
extern crate clap;
extern crate screeps_rs_ui;

use clap::{App, Arg};

fn main() {
    let matches = App::new("screeps-rs")
        .version(crate_version!())
        .author(&*crate_authors!().replace(":", "\n"))
        .about("Native client for the Screeps JavaScript MMO")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("enables verbose logging"),
        )
        .arg(
            clap::Arg::with_name("debug-modules")
                .short("d")
                .long("debug")
                .value_name("MODULE_PATH")
                .help("Enable verbose logging for a specific module")
                .long_help(
                    "Enables verbose debug logging for an individual rust module or path.\n\
                     For example, `--debug screeps_rs_ui::ui` will enable verbose logging for UI related code.\n\
                     \n\
                     Common modules you can use:\n\
                     - screeps_rs_network                   app network calling and result caching\n\
                     - screeps_rs_ui                        app glue and UI\n\
                     - screeps_rs_ui::window_management     event glue\n\
                     - screeps_rs_ui::rendering             game state rendering\n\
                     - screeps_api                          network result parsing\n\
                     - screeps_api::sockets                 websocket network result parsing\n\
                     - hyper                                HTTP client\n\
                     - ws                                   websocket client",
                )
                .takes_value(true)
                .multiple(true),
        )
        .get_matches();

    screeps_rs_ui::main(
        matches.is_present("verbose"),
        matches
            .values_of("debug-modules")
            .into_iter()
            .flat_map(|iter| iter),
    );
}
