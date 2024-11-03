# Solarscape

Solarscape is a **work in progress** multiplayer space survival, base building, and combat game.

*The name "Solarscape" is a potentially temporary title.*

As Solarscape is yet to even reach a closed alpha state, no builds or instructions for building are distributed, however
building an executable with the source code provided is trivial.

Keep up with development on [Discord](https://solarscape.astralchroma.dev/discord).

## A quick overview of the project structure

Solarscape is split into 3 programs:
- `client`:        This should be obvious, it is the game client used to play Solarscape.
- `sector-server`: Solarscape does not have a single game world, it is split into sectors hosted on individual servers,
                   this is mostly a decomposition thing, and partly "leave room to scale later" thing.
- `gateway`:       A Http Api responsible for everything other then sector game state, it manages accounts,
                   authentication, and brokers connections between clients and the sector servers.

Additionally there is a library crate (`shared`) used to contain code shared between all applications, what code is
available is controlled by features:
- `backend`: Code shared between backend applications, currently `gateway` and `sector-server`.
- `world`: Code shared between applications that interact with the game world, currently `client` and `sector-server`.

PostgreSQL is used for both data storage and messaging.
We have no plans to use a Redis/Redis-like service for the time being as PostgreSQL is sufficient.
