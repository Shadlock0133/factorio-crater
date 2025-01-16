simple rust project to analyze metadata of factorio mods

run with `cargo run --release -- -U` to download metadata of all mods
from mod portal (this will create `mods/` folder, which will weigh ~120MB),
then run `cargo run --release` to analyze which mods are "broken",
ie. can't be installed with dependencies in factorio versions <= 1.1
