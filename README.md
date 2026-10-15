# umamusume-race-simulator

An umamusume simulator that is verifiably accurate to in-game races, allowing you to load race replays and tinker with uma skills/stats to see how races would've played out if you'd prepared a little bit more - or maybe if you feel like you just got unlucky you want to run 1000x simulations with the same lobby to see if you should've won after all...

Or maybe you just want a stamina calculator that is finally 100% accurate with no caveats or estimations~

See https://github.com/kachi-dev/ulala-sim for a simulator that integrates uma-race-simulator

This project took a few days of work between myself and my top engineer (mr Opus 5.5) and his team of hallucinating sub-agents, and to be honest, that's the only reason this even exists - I don't have the time or arguably the skill to do this sort of thing myself but well I think this is interesting enough to share nonetheless~

Admittedly, this is probably just one of many upcoming race simulators, as AI has made reverse engineering trivial even for those with no decompilation/dissassembly experience. I imagine VMProtect/Themida stonks are rising at the moment, but I wouldn't be surprised if AI was even able to dump and reverse engineer virtualized code too...

## Accuracy

Checked frame by frame against 1838 real race replays:

| | |
|---|---|
| Races matching every frame | 1838/1838 (100%) |
| Same finish order | 1838/1838 (100%) |
| Exact HP at the goal, per uma | 100% |
| Simulation time | ~10 ms per race |

I suspect that, due to a relatively small sample size, some of the simulation rules work by accident, so there will likely be edge cases. I will keep collecting room match data to test against more races and fix these as they come up.

## Usage

Requires Rust (stable).

```sh
cargo build --release

# compare one saved race against its replay
./target/release/compare "Saved races/Champions meeting/x.json"

# compare a whole folder of saved races
./target/release/compare --corpus "Saved races"

cargo test
```

To use game data from somewhere other than `data/`, set its path at build time in `.cargo/config.toml`:

```toml
[env]
UMA_DATA_DIR = "/path/to/data"
```

After a game update, regenerate `data/master_sim.db` (and `data/names.json`, the display text Ulala syncs: umas, skills, races, Champions Meetings) from your install's `master.mdb`:

```sh
cargo run --release --bin extract_master -- <path to master.mdb>
```

Champions Meetings announced ahead of the Global client go in `data/cm_upcoming.json` by hand, rows like names.json's `cm`: `[number, name, null, course_set, race_instance, ground, weather, season, time]`. The date stays null until the game has it; race_instance borrows an earlier CM's on the same course (a G1, for grade conditions) while the real one isn't in `master.mdb`. Ulala ignores a row once names.json has that number, so it can be deleted any time after.

Differences between the community race mechanics doc and what the game actually does: [docs/race_mech_diffs.md](docs/race_mech_diffs.md).

## License

The source code is [MIT](LICENSE). The game data under `data/` is extracted from Umamusume: Pretty Derby and remains the property of Cygames, Inc.; it is not covered by the license.
