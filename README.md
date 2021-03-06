screeps-rs
==========
[![Linux Build Status][travis-image]][travis-builds]
[![Windows Build Status][appveyor-image]][appveyor-builds]

WIP native screeps client using [Rust] and [conrod].

Screeps is a true programming MMO where users upload JavaScript code to power their online empires.

![map rendering screenshot][map-image]

![zoomed out screenshot][strategic-view]

This client is built on three main projects:
- [`rust-screeps-api`] implements HTTP calls, endpoints and json result parsing
- [`screeps-rs-network`] implements result caching, keeping track of http and websocket connections, and providing an 'event' api
- [`screeps-rs-ui`] implements rendering and a UI

[`rust-screeps-api`] can:

- Connect to screeps.com with HTTP calls and websocket connections
- Authenticate
- Retrieve room terrain, map room overviews, basic user information and room details.

[`screeps-rs`] can:

- Connect to screeps.com
- Login through a UI
- Render basic room terrain, map view, and information of the logged in user.

Eventually, this will be able to connect to both the [official server][screeps] and any [private server][screeps-os] instances run by users.

Running:
- If you're on Ubuntu 17.10+, or on another Wayland Linux: (see [glutin#949])
  - install "libegl1-mesa-dev"
  - soft-link `libwayland-egl.so.1` to `libwayland-egl.so` in your system's lib dir. On ubuntu:
    
    ```
    cd /usr/lib/x86_64-linux-gnu/
    sudo ln -s libwayland-egl.so.1 libwayland-egl.so
    ```

Neighbor projects:

- APIs:
  - [`python-screeps`] implements a compact screeps API interface in python
  - [`node-screeps-api`] implements an interface for the screeps API in node.js
- Clients:
  - [`ricochet1k/screeps-client`] implements a full screeps room viewer in browser JavaScript
  - [`ags131/screeps-client`] implements a slightly-less-full screeps room viewer in browser JavaScript
  - [`screeps-silica`] is directly connected to screeps-rs, using Scala to accomplish the same goals
  - [`Screeps3D`] is a native 3D screeps client built using Unity3D

[`screeps-rs`] uses the `Akashi` font. It is included with permission from [Ten by Twenty][ten-by-twenty].

[travis-image]: https://travis-ci.org/daboross/screeps-rs.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/screeps-rs
[appveyor-image]: https://ci.appveyor.com/api/projects/status/github/daboross/screeps-rs?branch=master&svg=true
[appveyor-builds]: https://ci.appveyor.com/project/daboross/screeps-rs
[rust]: https://www.rust-lang.org/
[conrod]: https://github.com/PistonDevelopers/conrod/
[`rust-screeps-api`]: https://github.com/daboross/rust-screeps-api
[`screeps-rs-network`]: network/
[`screeps-rs-ui`]: ui/
[`screeps-rs`]: https://github.com/daboross/screeps-rs
[`python-screeps`]: https://github.com/screepers/python-screeps/
[`node-screeps-api`]: https://github.com/screepers/node-screeps-api
[`screeps-silica`]: https://github.com/daboross/screeps-silica/
[`ricochet1k/screeps-client`]: https://github.com/ricochet1k/screeps-client
[`ags131/screeps-client`]: https://github.com/ags131/screeps-client
[screeps]: https://screeps.com
[screeps-os]: https://github.com/screeps/screeps/
[ten-by-twenty]: http://tenbytwenty.com/
[map-image]: docs/map-render.png
[strategic-view]: docs/strategic-view.png
[tomaka/glutin#949]: https://github.com/tomaka/glutin/issues/949
[`Screeps3D`]: https://github.com/bonzaiferroni/Screeps3D
