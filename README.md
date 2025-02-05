# factorio-crater

simple rust project to analyze metadata of factorio mods and manage mods

## install

install with `cargo install --path .`

## usage

- `factorio-crater -U` to download metadata of all mods from mod portal (this
will create `mods/` folder, which will weigh ~120MB)

- `factorio-crater run file.lua` will run lua script with global `mods` letting
you access mods metadata

- `factorio-crater download -f /path/to/factorio sodaaaaa` to download mods into
factorio instance (requires to be logged into factorio account in that instance)

- (WIP) `factorio-crater` or `factorio-crater gui` to launch gui for managing
mods
