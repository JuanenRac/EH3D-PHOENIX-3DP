# How the program fits with the EH3D server

The EH3D server (`eh3d-server`) and the programs around it are the centre of the suite: the machines, the jobs and the permissions live in the server, and the programs are clients of it. PHOENIX-3DP is one more client. It can also work on its own, talking straight to a printer, but that stand-alone mode is an option that is off by default and has no shared queue and no permissions.

## The settings of the link

They are the same keys, with the same ranges and defaults, as the common client of the suite (`eh3d-serverclient`, which this program uses: `p3dp-server-bridge` hands every key to it, so a value the suite refuses is refused here too). They are read from the settings file (`--config FILE`, see `examples/phoenix-3dp.cfg`).

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

A connection that could not be made is tried again for any request, because nothing was sent. A time-out, a broken connection or a 502, 503 or 504 is tried again only for GET, PUT and DELETE; a POST (a job) is never sent twice by the client. The pauses double up to two seconds. For the server they are the rules of the common client; for a printer reached straight (Moonraker, OctoPrint) they are in `p3dp-link/src/retry.rs`, over `TcpTransport` (plain HTTP/1.1, no TLS, a limit of time to connect and to each read, a limit on the head and the body of an answer, and no byte that could start another request), and tested with a fixed seed against a socket of the same computer.

## The printer plugin

The server serves a plugin for 3D printers (`printer3d`) under the generic routes of its machines and jobs. The client of `p3dp-server-bridge` speaks these routes; it has been tried against a double of them in memory, never against a running server.

| Route | Use |
|---|---|
| `GET /api/v1/health` | is the server there (no token is sent) |
| `POST /api/v1/auth/login`, `refresh`, `logout` | the login of the program, with the application `phoenix-3dp` (the server needs that name in `auth_extra_apps`, or a token of a program) |
| `GET /api/v1/work/machines` | the machines of every plugin; the client keeps the ones whose plugin is `printer3d`, with their model, connection, limits of temperature and whether they are latched |
| `GET /api/v1/work/machines/{id}/temperatures`, `.../layer` | the heaters with their limits, and the layer a print is at |
| `POST /api/v1/work/machines` | an administrator makes a machine for a profile of the catalog (the model that the server knows, or `generic`, the conservative limits) |
| `POST /api/v1/work/jobs` | puts a program in the queue; the program is read first and a temperature over the limit is refused with its line and nothing is sent |
| `GET /api/v1/work/jobs`, `GET /api/v1/work/jobs/{id}` | the queue and one job |
| `POST /api/v1/work/jobs/{id}/start`, `pause`, `resume`, `cancel` | what a person does to a job |
| `PUT /api/v1/work/machines/{id}/temperatures` | a target for one heater (`hotend`, `bed` or `chamber`); checked here against the stricter limits first, and again by the server (`422 temperature_over_limit`, `422 bad_heater`) |
| `GET /api/v1/work/machines/{id}/compat/moonraker`, `.../compat/octoprint` | the state in the shape of another program's answer; the server gives it only when the machine has the setting `compat` (otherwise `501 not_supported`) |
| `POST /api/v1/work/machines/{id}/emergency-stop` | heaters off and motors released at once; the machine stays latched until an administrator calls `.../release` |

The limits of temperature are checked twice: by the program with the stricter of its own profile and the limits the server reports for the machine, and by the server, which refuses a program over its limits whoever sends it (`422 program_over_limit`).

## The queue never starts a job by itself

The program only ever *queues* a job. Starting it is a separate call that needs a `Confirmation`, a value that only the call standing for a person's click can make, so code that has the id of a job cannot move a printer. Whether the server starts queued jobs by itself (`auto_start_queue` of the machine) is the policy of the server, off by default, and the program never asks for it.

## What the server will own

The connection to each printer (one owner per printer, so the clients never open their own), the credentials of the printers (encrypted at rest; the clients only see identifiers), the queue, the history, the camera relay and the permissions per user and per printer. Slicing may also run in the server, as an option. The queue, the plugin and the limits are in the server; the credentials of the printers and the camera relay are still to come.
