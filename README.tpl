# Ratatui-image

[![CI Badge]][CI]
[![Crate Badge]][Crate]
[![Docs Badge]][Docs]

[CI Badge]: https://img.shields.io/github/actions/workflow/status/benjajaja/ratatui-image/ci.yaml?style=flat-square&logo=github
[CI]: https://github.com/benjajaja/ratatui-image/actions?query=workflow%3A
[Crate Badge]: https://img.shields.io/crates/v/ratatui-image?logo=rust&style=flat-square
[Crate]: https://crates.io/crates/ratatui-image
[Docs Badge]: https://img.shields.io/docsrs/ratatui-image?logo=rust&style=flat-square
[Docs]: https://docs.rs/ratatui-image/latest/ratatui_image/index.html

### Showcase:

![Recording](./assets/Recording.gif)

{{readme}}

### Compatibility matrix

Compatibility and QA:

Terminal  | Protocol | Fixed | Notes
----------|----------|-------|-------
Xterm     | `Sixel`  | ✔️     | Run with `-ti 340` to make sure sixel support is enabled.
Foot      | `Sixel`  | ✔️     | Wayland.
Kitty     | `Kitty`  | ✔️     |
Wezterm   | `iTerm2` | ✔️     | Also would support `Sixel` and `Kitty`, but only `iTerm2` actually works bug-free.
Alacritty | `Sixel`  | ❌    | [There is a sixel fork](https://github.com/microo8/alacritty-sixel), but it's probably never getting merged, and does not clear graphics.
iTerm2    | `iTerm2` | ❔    | Feedback from mac users wanted.
Konsole   | `Sixel`  | ❌    | [Possibly fixed in 24.12](https://bugs.kde.org/show_bug.cgi?id=456354)
Contour   | `Sixel`  | ❌    | Does not clear graphics.
ctx       | `Sixel`  | ❌    | Buggy.
Blackbox  | `Sixel`  | ❔    | Untested.

A basic [screenshot test](./assets/screenshot_xterm.png) is run with xterm on Xvfb in the CI (or `cargo make screenshot-xvfb && cargo make screenshot-diff`).

Halfblocks should work in all terminals, even if the font size could not be detected, with a 4:8 pixel ratio.

### Projects that use ratatui-image

* [iamb](https://github.com/ulyssa/iamb)
  A matrix client with vim keybindings.
* [joshuto](https://github.com/kamiyaa/joshuto)
  A terminal file manager that can preview images.
* [Aerostream](https://github.com/shigepon7/aerostream)
  A Bluesky client using EventStream.

### Comparison

* [viuer](https://crates.io/crates/viuer)
  Renders graphics in different terminals/protocols, but "dumps" the image, making it difficult to
  work for TUI programs.
  The terminal protocol guessing code has been adapted to rustix, thus the author of viuer is
  included in the copyright notice.
* [yazi](https://github.com/sxyazi/yazi)
  Not a library but a terminal file manager that implementes many graphics protocols and lets you
  preview images in the filesystem.
* [Überzug++](https://github.com/jstkdng/ueberzugpp)
  CLI utility that draws images on terminals by using X11/wayland child windows, sixels, kitty,
  and/or iterm2 protocols (any means necessary). There exists several wrapper or bindings crates.
  More battle-tested but essentially stateful, which makes it hard to use with immediate-mode.

### Contributing

PRs and issues/discussions welcome!

You can run an aproximation of the CI with `cargo make ci`. I must manually approve CI runs for new 
PRs to prevent github-action attacks. The demo is useful to test that everything works correctly.

License: {{license}}

