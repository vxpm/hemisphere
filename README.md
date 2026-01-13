# lazuli

A work-in-progress GameCube emulator :)

- [Status](#status)
- [Building](#building)
- [Usage](#usage)
- [Contributing](#contributing)
- [Random Q&A](#random-qa)

# Status

`lazuli` is still very much a toy, but it's able to boot multiple commercial games and lots of
homebrew. Here's a very small list of games that are frequently tested and go in game with decent graphics:

- Super Mario Sunshine
- Luigi's Mansion
- The Legend of Zelda: The Wind Waker
- Crash Bandicoot: The Wrath of Cortex
- WarioWare, Inc.: Mega Party Game$!

On a more technical note, here's what `lazuli` currently offers:

- `cranelift` based PowerPC JIT compiler
- `cranelift` based JIT vertex parser compiler
- DSP LLE interpreter
- `wgpu` based renderer backend
- `cpal` based audio backend
- `wesl` based shader generator/compiler
- HLE IPL used to boot games
- A very simple UI for debugging

# Building

To build lazuli, you'll need the latest nightly rust toolchain (which can be obtained through `rustup`) and the `just` command runner.

First, run `just ipl-hle build` to build the ipl-hle binary, which is embedded into the lazuli executable. This should generate `ipl-hle.dol` inside a `local/` directory in the workspace.

Then, build the main lazuli app by executing `cargo build` (with any optional flags you might want, such as `--release`). This should produce an `app` executable inside `target/chosen_profile`.

# Usage

## Running a game

Once you have a `lazuli` executable (either by building it or by grabbing one of the nightly releases), you can run it in the terminal with a path to the `.iso` file you want to run:

```sh
lazuli --iso path/to/gamecube/game.iso
```

You do not need an IPL ROM (the "bios") to run games, but some games might use it's embedded font (in which case you won't see anything if you don't have one). To pass an IPL:

```sh
lazuli --ipl path/to/ipl.bin --iso path/to/gamecube/game.iso
```

You can also pass `--force-ipl` to skip the high-level emulation of the IPL (IPL-HLE) and instead use the provided rom to boot. Beware this currently has issues and will likely not work.

For more CLI options, `--help` is your friend.

## Inputs

Currently, only gamepads are supported (i.e. there's no keyboard based input). When a gamepad is detected, it is automatically set as the active gamepad - there's no need to do anything special.

## Debugging

The UI has many features that are useful for debugging. With it, you can set breakpoints, watch memory variables, analyze call stacks and more. To open windows, click the `view` button in the top-left corner of the screen (it's in the top bar).

# Contributing

Contributions are very welcome! You do not need to be an expert on the GameCube's internals to contribute, there's multiple other ways you could help:

- Improving UI
- Optimizing performance
- Fixing bugs
- Documenting stuff
- And more!

If you're interested, **please** read [the contribution guidelines](./CONTRIBUTING.md) before getting started.

# Random Q&A

Here's some random questions and their answers. I'd call this a FAQ but no one has ever asked these questions so I'm not sure it would be appropriate :p

## Is there any reason I should use this over Dolphin?

No, not yet. Dolphin is a thousand times more mature and what you should use if you want to actually play games.

## Is this a reimplementation of Dolphin in Rust?

No, this is built from the ground up. No dolphin code is reused/stolen/whatever.

## Does this support Wii?

Not yet. It's a long-term goal, since the Wii is very similar to the GameCube. There's currently no infrastructure for it, though.

## What is `hemisphere`?

The old name of this project. I renamed it to `lazuli` because it's cute.
