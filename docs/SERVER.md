# How the program fits with the EH3D server

The EH3D server (`eh3d-server`) and the programs around it are the centre of the suite: the machines, the jobs and the permissions live in the server, and the programs are clients of it. PHOENIX-3DP is one more client. It can also work on its own, talking straight to a printer, but that stand-alone mode is an option that is off by default and has no shared queue and no permissions.

## The settings of the link

They are the same keys, with the same ranges and defaults, as the common client of the suite (`eh3d-serverclient`). They are read from the settings file (`--config FILE`, see `examples/phoenix-3dp.cfg`).

| Key | Range | Default | Meaning |
|---|---|---|---|
| `server_address` | `host:port`, or empty | empty | the server; empty means there is none and nothing is sent |
| `server_token` | text without spaces | empty | the token of the program; never shown, never logged |
| `server_machine` | text | empty | the machine the jobs are for |
| `server_timeout_s` | 1 to 120 | 15 | how long one try may take |
| `server_retries` | 0 to 5 | 2 | how many more tries after a failure on the network |
| `server_mdns` | `on` or `off` | `off` | look for the server on the local network (nothing announces it yet) |
| `server_max_answer_mib` | 1 to 256 | 8 | the most MiB read from one answer |
| `direct_mode` | `on` or `off` | `off` | the stand-alone mode: straight to a printer when there is no server |

A value out of its range is an error that names the key and the line, and changes nothing. With no address, no discovery and no stand-alone mode, nothing is sent anywhere. A warning is given when the token would travel without encryption to an address outside the local network.

## The retry rules

A connection that could not be made is tried again for any request, because nothing was sent. A time-out, a broken connection or a 502, 503 or 504 is tried again only for GET, PUT and DELETE; a POST (a job) is never sent twice by the client. The pauses double up to two seconds with a jitter. The rules are in `p3dp-link/src/retry.rs` and tested with a fixed seed.

## The printer plugin: a draft

The server is to have a plugin for printers (`printer3d`). The routes below are the draft the thin client of `p3dp-server-bridge` follows; they are served by the server in memory of the tests and not by the real server yet.

| Route | Use |
|---|---|
| `GET /api/v1/health` | is the server there (no token is sent) |
| `GET /api/v1/printer3d/printers` | the printers and a summary of their state |
| `GET /api/v1/printer3d/printers/{id}/state` | the normalized state of one printer |
| `GET /api/v1/printer3d/queue` | the queue |
| `POST /api/v1/printer3d/queue` | put a job in the queue |

## The queue never starts a job by itself

Every request to the queue says `auto_start: false` and `confirm_before_start: true`. Whether the server may start jobs without a person is the policy of the server, off by default; a server whose policy is off answers 403 to a request that asks for it, and the program never asks.

## What the server will own

The connection to each printer (one owner per printer, so the clients never open their own), the credentials of the printers (encrypted at rest; the clients only see identifiers), the queue, the history, the camera relay and the permissions per user and per printer. Slicing may also run in the server, as an option. None of this exists in the server yet; the roadmap says when.
