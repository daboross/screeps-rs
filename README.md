screeps-rs
==========
[![Linux Build Status][travis-image]][travis-builds]
[![Windows Build Status][appveyor-image]][appveyor-builds]

WIP native screeps client using [Rust] and [conrod].

Screeps is a true programming MMO where users upload JavaScript code to power their online empires.

![terrain rendering screenshot][terrain-image]

This client is built on two main projects:
- [`rust-screeps-api`] implements networking and parsing of network results
- [`screeps-rs`] implements rendering and a UI

[`rust-screeps-api`] can:

- Connect to screeps.com with HTTP calls and websocket connections
- Authenticate
- Retrieve room terrain, map room overviews, basic user information and some room details.

[`screeps-rs`] can:

- Connect to screeps.com
- Login through a UI
- Render basic room terrain and user information.

Eventually, this will be able to connect to both the [official server][screeps] and any [private server][screeps-os] instances run by users.

[`screeps-rs`] uses the `Akashi` font. It is included with permission from [Ten by Twenty][ten-by-twenty].

[travis-image]: https://travis-ci.org/daboross/screeps-rs.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/screeps-rs
[appveyor-image]: https://ci.appveyor.com/api/projects/status/github/daboross/screeps-rs?branch=master&svg=true
[appveyor-builds]: https://ci.appveyor.com/project/daboross/screeps-rs
[rust]: https://www.rust-lang.org/
[conrod]: https://github.com/PistonDevelopers/conrod/
[`rust-screeps-api`]: https://github.com/daboross/rust-screeps-api
[`screeps-rs`]: https://github.com/daboross/screeps-rs
[screeps]: https://screeps.com
[screeps-os]: https://github.com/screeps/screeps/
[ten-by-twenty]: http://tenbytwenty.com/
[terrain-image]: docs/terrain-render.png
