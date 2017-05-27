screeps-rs
==========
[![Build Status][travis-image]][travis-builds]

WIP native screeps client using [Rust] and [conrod].

Screeps is a true programming MMO where users uploading JavaScript code to power their online empires.

![terrain rendering screenshot][terrain-image]

`screeps-rs` is built with two main projects: [`rust-screeps-api`] implements networking and parsing of network results, and this repository implements a UI on top of that.

`screeps-rs` can:

- Connect to screeps.com
- Successfully authenticate
- Render basic room terrain and user information.

`rust-screeps-api` can:

- Connect to screeps.com with HTTP calls and websocket connections
- Authenticate
- Retrieve room terrain, map room overviews, basic user information and some room details.

Eventually, this will be able to connect to both the [official server][screeps] and any [private server][screeps-os] instances run by users.

`screeps-rs` uses the `Akashi` font. It is included with permission from [Ten by Twenty][ten-by-twenty].

[travis-image]: https://travis-ci.org/daboross/conrod-testing.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/conrod-testing
[rust]: https://www.rust-lang.org/
[conrod]: https://github.com/PistonDevelopers/conrod/
[rust-screeps-api]: https://github.com/daboross/rust-screeps-api
[screeps]: https://screeps.com
[screeps-os]: https://github.com/screeps/screeps/
[ten-by-twenty]: http://tenbytwenty.com/
