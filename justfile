export RUSTDOCFLAGS := "-Zunstable-options --show-type-layout --generate-link-to-definition --default-theme dark"

# Lists all recipes
list:
    @just --list

# Opens the documentation of the crate (including private items)
doc:
    cargo doc --open
