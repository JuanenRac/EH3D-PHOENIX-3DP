# EH3D-PHOENIX-3DP: requirements

This register is the single source of the state of each capability: `STATUS.md` and the README files cite it and never state a state that it does not. A state is one of `planned`, `prototype`, `implemented` and `validated`. `validated` means that a person has run the capability on a real machine and the evidence of the manifest says `bench` or `field`; nothing here is validated, because nothing of this program has run on a real printer, a real server or a screen: the printers are simulators (in memory and on a socket of the same computer), the server is a double, and every profile of the catalog says `verified = false`.

The register has 142 requirements: 128 implemented, 8 prototype, 6 planned and 0 validated. By progress: 128 done, 8 partial, 2 pending and 4 blocked. High coverage is not proof of correctness, so each row says how it is checked: the test that names it, and what it shows.

## How to read the register

* **ID:** `P3DP-NNN`, never reused and never renumbered; a new capability takes the next number.
* **State:** `implemented` is code that is complete and tested on a computer, `prototype` is part of the capability, `planned` is not written.
* **Progress:** `done`, `partial`, `pending` (can be written without hardware) or `blocked` (waits for hardware or for another program of the suite).
* **Evidence:** `simulator` when the test runs against a simulated printer in memory or on a socket of the same computer, a broker in memory or a double of the server, `host-test` when it checks a calculation, a reader or a screen-less piece, `none` when nothing is written yet.
* **Test:** the acceptance criterion: `file::name` is the test `name` of `tests/file.rs` (`src/file::name` is a test inside `src/file.rs`), which must exist. For a capability that is not written the cell says `planned:` and the name of the test that will show it; the day that test exists, the row must change.
* **Limits:** what is not done or not proven; a row that is `partial` or `blocked` always says it.

The check is `tests/reqs.rs`: it reads this table, finds each test and fails when a state, a test or a count does not agree.

## Models and file formats

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-001 | A binary STL makes a round trip, an ASCII STL is read, and a header that promises too much is refused before allocating. | implemented | done | host-test | `crates/p3dp-formats/src/stl::a_header_that_promises_too_much_is_refused_before_allocating` | - |
| P3DP-002 | An OBJ makes an exact round trip and its limits apply. | implemented | done | host-test | `crates/p3dp-formats/src/obj::round_trip_is_exact` | - |
| P3DP-003 | What the program writes as 3MF it reads back exactly, and a package of a slicer with deflate and a transform is read. | implemented | done | host-test | `crates/p3dp-formats/src/threemf::what_the_program_writes_it_reads_back_exactly` | - |
| P3DP-004 | Damaged 3MF packages never panic. | implemented | done | host-test | `crates/p3dp-formats/src/threemf::damaged_packages_never_panic` | - |
| P3DP-005 | A zip bomb and too many entries are refused, and unsafe names are refused. | implemented | done | host-test | `crates/p3dp-formats/src/zip::a_bomb_and_too_many_entries_are_refused` | - |
| P3DP-006 | The output limit of the inflater stops a bomb. | implemented | done | host-test | `crates/p3dp-formats/src/inflate::the_output_limit_stops_a_bomb` | - |
| P3DP-007 | An entity declaration in XML is refused. | implemented | done | host-test | `crates/p3dp-formats/src/xml::an_entity_declaration_is_refused` | - |
| P3DP-008 | The content wins over a wrong file name when a format is detected. | implemented | done | host-test | `crates/p3dp-formats/src/detect::the_content_wins_over_a_wrong_name` | - |
| P3DP-009 | Every format makes a round trip through read and write. | implemented | done | host-test | `crates/p3dp-formats/src/detect::every_format_round_trips_through_read_and_write` | - |
| P3DP-010 | A cube is a good mesh, a missing face is an open mesh, and inverted faces are noticed. | implemented | done | host-test | `crates/p3dp-core/src/mesh::a_missing_face_is_an_open_mesh` | - |
| P3DP-011 | Random garbage never panics the mesh checks. | implemented | done | host-test | `crates/p3dp-core/src/mesh::random_garbage_never_panics_the_checks` | - |
| P3DP-012 | A file cut at every position is an error or a mesh. | implemented | done | host-test | `tests/fuzz::a_file_cut_at_every_position_is_an_error_or_a_mesh` | - |
| P3DP-013 | The metadata of binary G-code is listed and its thumbnails can be cut out. | implemented | done | host-test | `crates/p3dp-formats/src/bgcode::a_file_with_every_kind_of_block_is_listed_and_its_text_is_read` | - |
| P3DP-014 | A thumbnail that is packed, cut short or not the picture it says is not handed out. | implemented | done | host-test | `crates/p3dp-formats/src/bgcode::a_thumbnail_that_is_packed_cut_short_or_not_the_picture_it_says_is_not_handed_out` | - |
| P3DP-015 | The projects of other programs (Prusa, Bambu and Orca families) are read for their objects, plates and a few settings. | prototype | partial | host-test | `crates/p3dp-formats/src/project3mf::a_project_of_the_prusa_family_gives_its_objects_and_settings` | Read only and from the layout of the files as it looks in a package; the settings outside a short list are not read and nothing is written back. |
| P3DP-016 | An open package of resin layers is read by its headers with an estimate of the exposure. | prototype | partial | host-test | `crates/p3dp-formats/src/resin::a_package_of_layers_gives_what_its_config_says_without_decoding_an_image` | No layer image is decoded and no file is written; the closed formats of the makers are refused. |
| P3DP-017 | The exposure of a resin layer follows a working curve and what goes past a limit is refused with the layer and the setting. | implemented | done | host-test | `crates/p3dp-formats/src/resin_control::what_goes_past_a_limit_is_refused_with_the_layer_and_the_setting` | - |
| P3DP-018 | A model becomes a program that a simulated printer runs, and a chain of conversions keeps the volume. | implemented | done | host-test | `tests/end_to_end::a_chain_of_conversions_keeps_the_volume` | - |

## Printers, filaments and limits

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-019 | The catalog holds a hundred and twenty models of both technologies and every profile is coherent. | implemented | done | host-test | `crates/p3dp-profiles/src/catalog::the_catalog_has_a_hundred_and_twenty_models_of_both_technologies` | - |
| P3DP-020 | Every profile of the catalog says that it is not verified yet. | implemented | done | host-test | `crates/p3dp-profiles/src/catalog::nothing_is_verified_yet` | - |
| P3DP-021 | The limits of a printer are the ones its maker publishes, and the owner can edit them. | prototype | partial | host-test | `crates/p3dp-profiles/src/profile::raising_a_limit_past_the_maker_is_allowed_and_told_and_makes_the_limits_the_owners` | No limit has been compared with a printer; a wrong limit in the catalog is a bug to report. |
| P3DP-022 | A temperature over the limit is refused and never lowered. | implemented | done | host-test | `crates/p3dp-core/src/machine::a_temperature_over_the_limit_is_refused_not_lowered` | - |
| P3DP-023 | A pair of printer and filament that does not fit is a list of reasons and never a lowered temperature. | implemented | done | host-test | `crates/p3dp-profiles/src/filament::a_pair_that_does_not_fit_is_a_list_of_reasons_and_never_a_lowered_temperature` | - |
| P3DP-024 | A variant with fibre fill is refused on a nozzle that is not hard. | implemented | done | host-test | `crates/p3dp-profiles/src/filament::the_variants_with_fibre_fill_wear_a_brass_nozzle_and_are_refused_on_one` | - |
| P3DP-025 | A resin goes only in a resin printer. | implemented | done | host-test | `crates/p3dp-profiles/src/resin::a_resin_goes_only_in_a_resin_printer` | - |
| P3DP-026 | The check before printing compares the temperatures with the range of the maker and the flow of the slice with the flow of the filament. | implemented | done | host-test | `src/preflight::a_temperature_outside_the_range_of_the_maker_is_a_warning_with_the_figures` | - |
| P3DP-027 | A profile makes a round trip through its file and a mistake names the key. | implemented | done | host-test | `crates/p3dp-profiles/src/profile::a_profile_file_names_what_is_wrong` | - |
| P3DP-028 | The limits of the program have conservative defaults and a key out of range names the line. | implemented | done | host-test | `crates/p3dp-core/src/limits::a_key_out_of_range_names_the_line` | - |
| P3DP-029 | The filaments are looked for by maker, material and several words. | implemented | done | host-test | `src/lists::the_filaments_are_looked_for_by_maker_and_by_material_with_several_words` | - |

## The slicer

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-030 | A cube is cut in one counter-clockwise square of the right area. | implemented | done | host-test | `crates/p3dp-slicer/src/cut::a_cube_is_cut_in_one_counter_clockwise_square_of_the_right_area` | - |
| P3DP-031 | A mesh that is not closed gives an error and a huge cut is stopped. | implemented | done | host-test | `crates/p3dp-slicer/src/cut::a_mesh_that_is_not_closed_gives_an_error_and_a_huge_cut_is_stopped` | - |
| P3DP-032 | A cube gets the layers, the perimeters and the infill it should. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::a_cube_gets_the_layers_the_perimeters_and_the_infill_it_should` | - |
| P3DP-033 | A solid cube lays down the plastic of its volume. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::a_solid_cube_lays_down_the_plastic_of_its_volume` | - |
| P3DP-034 | What cannot be printed is refused and never changed. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::what_cannot_be_printed_is_refused_and_never_changed` | - |
| P3DP-035 | The gyroid waves stay inside, and more infill is more plastic. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::the_gyroid_waves_stay_inside_and_more_infill_is_more_plastic` | - |
| P3DP-036 | Supports hold the plate and there is no bridge over them. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::supports_hold_the_plate_and_there_is_no_bridge_over_them` | - |
| P3DP-037 | A bridge that is too long is refused. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::a_skin_over_nothing_is_a_bridge_and_a_bridge_that_is_too_long_is_refused` | - |
| P3DP-038 | The skirt, the brim and the raft keep to the properties on shapes from a fixed seed. | implemented | done | host-test | `crates/p3dp-slicer/tests/properties::skirt_brim_and_raft_keep_to_the_properties_on_shapes_from_a_fixed_seed` | - |
| P3DP-039 | A vase is one wall with a solid base and nothing inside. | implemented | done | host-test | `crates/p3dp-slicer/tests/properties::a_vase_is_one_wall_with_a_solid_base_and_nothing_inside` | - |
| P3DP-040 | Shapes from a fixed seed keep to the properties of the slicer in every dialect. | implemented | done | host-test | `crates/p3dp-slicer/tests/properties::shapes_from_a_fixed_seed_keep_to_the_properties` | - |
| P3DP-041 | The head lifts on a travel that retracts and never past the printer. | implemented | done | host-test | `crates/p3dp-slicer/src/plan::the_head_lifts_on_a_travel_that_retracts_and_never_past_the_printer` | - |
| P3DP-042 | The estimate of the time with acceleration lies between the plain time and the stop-and-go time. | prototype | partial | host-test | `crates/p3dp-slicer/src/timing::the_estimate_lies_between_the_plain_time_and_the_stop_and_go_time` | Checked for consistency; never compared with the time a real printer takes. |
| P3DP-043 | The material, the weight and the cost add up to the plan. | implemented | done | host-test | `crates/p3dp-slicer/src/report::the_material_adds_up_to_the_plan_and_the_weight_follows_the_density` | - |
| P3DP-044 | Settings that do not go together are refused, not adjusted. | implemented | done | host-test | `crates/p3dp-slicer/src/config::settings_that_do_not_go_together_are_refused_not_adjusted` | - |
| P3DP-045 | A print with several materials changes tool between layers and cleans the nozzle in a purge tower, and a tower that does not fit, touches the part or is too small is refused. | implemented | done | host-test | `crates/p3dp-slicer/src/multi::a_two_material_print_gets_a_purge_tower` | Tools change between layers only; nothing is tried on a printer. |
| P3DP-046 | The top is ironed with very little plastic and the pressure advance is written in the command of each firmware, both with their ranges checked. | implemented | done | host-test | `crates/p3dp-slicer/tests/quality::ironing_goes_over_the_top_skin_with_little_plastic` | The top layer only is ironed; the value of the pressure advance is the person's, not a calibration. |

## G-code and its check

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-047 | A temperature over the limit writes nothing. | implemented | done | host-test | `crates/p3dp-gcode/src/writer::a_temperature_over_the_limit_writes_nothing` | - |
| P3DP-048 | Moves stay in the volume of the printer. | implemented | done | host-test | `crates/p3dp-gcode/src/writer::moves_stay_in_the_volume` | - |
| P3DP-049 | Klipper uses its macros with parameters. | implemented | done | host-test | `crates/p3dp-gcode/src/writer::klipper_uses_the_macros_with_parameters` | - |
| P3DP-050 | Bambu needs a package and reports progress. | implemented | done | host-test | `crates/p3dp-gcode/src/writer::bambu_reports_progress_and_needs_a_package` | - |
| P3DP-051 | A chamber needs a heated chamber. | implemented | done | host-test | `crates/p3dp-gcode/src/writer::a_chamber_needs_a_heated_chamber` | - |
| P3DP-052 | What the writer writes always passes the checker, and what the writer refuses the checker also finds. | implemented | done | host-test | `crates/p3dp-gcode/src/check::what_the_writer_refuses_the_checker_also_finds` | - |
| P3DP-053 | The writer accepts exactly what the model of the printer allows. | implemented | done | host-test | `tests/safety::the_writer_accepts_exactly_what_the_model_allows` | - |
| P3DP-054 | The checker agrees with the writer on every printer of the catalog. | implemented | done | host-test | `tests/safety::the_checker_agrees_with_the_writer_on_every_printer` | - |
| P3DP-055 | The command line slices, reports, writes and refuses. | implemented | done | host-test | `src/cli::slice_reports_writes_and_refuses` | - |
| P3DP-056 | A garbage G-code never panics the checker. | implemented | done | host-test | `crates/p3dp-gcode/src/check::garbage_never_panics` | - |

## Links to printers

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-057 | A whole job from upload to cancel runs against a simulated printer. | implemented | done | simulator | `crates/p3dp-link/tests/sim_drivers::a_whole_job_from_upload_to_cancel` | - |
| P3DP-058 | An emergency stop shuts the printer down in the simulator. | implemented | done | simulator | `crates/p3dp-link/tests/sim_drivers::an_emergency_stop_shuts_the_printer_down` | - |
| P3DP-059 | A random storm of commands never goes past the limits. | implemented | done | simulator | `crates/p3dp-link/tests/sim_drivers::a_random_storm_of_commands_never_goes_past_the_limits` | - |
| P3DP-060 | Raw G-code is off unless switched on and keeps to the limits. | implemented | done | simulator | `crates/p3dp-link/src/driver::raw_gcode_needs_the_switch_and_keeps_to_the_limits` | - |
| P3DP-061 | A temperature is refused by the driver and not cut. | implemented | done | simulator | `crates/p3dp-link/src/driver::temperatures_are_refused_not_cut` | - |
| P3DP-062 | Moonraker is driven over a socket of the same computer. | implemented | done | simulator | `crates/p3dp-link/tests/tcp_local::moonraker_over_a_socket` | - |
| P3DP-063 | OctoPrint is driven over a socket of the same computer. | implemented | done | simulator | `crates/p3dp-link/tests/tcp_local::octoprint_over_a_socket` | - |
| P3DP-064 | Retries hide a flaky network for reads but never repeat a POST. | implemented | done | simulator | `crates/p3dp-link/tests/sim_drivers::retries_hide_a_flaky_network_for_reads_but_never_repeat_a_post` | - |
| P3DP-065 | Nothing that could start another request is ever written to the connection. | implemented | done | simulator | `crates/p3dp-link/src/tcp::nothing_that_could_start_another_request_is_written` | - |
| P3DP-066 | TLS is refused with the reason and bad addresses never connect. | implemented | done | simulator | `crates/p3dp-link/tests/tcp_local::tls_is_refused_with_the_reason_and_bad_addresses_never_connect` | - |
| P3DP-067 | The packets of MQTT are the bytes of the examples and every packet comes back as it went. | implemented | done | simulator | `crates/p3dp-link/src/mqtt::the_packets_are_the_bytes_of_the_examples` | - |
| P3DP-068 | Only the certificate that was pinned is taken, and no pin means no link. | implemented | done | simulator | `crates/p3dp-link/src/pin::only_the_certificate_that_was_pinned_passes_and_no_pin_means_no_link` | - |
| P3DP-069 | The local link of Bambu runs against a broker in memory and a wrong code is refused. | prototype | partial | simulator | `crates/p3dp-link/src/bambu::the_whole_exchange_runs_against_a_broker_in_memory_and_a_wrong_code_is_refused` | Bytes and text against a broker made in memory; there is no TLS to open the real connection and nothing has been tried on a Bambu printer. |
| P3DP-070 | The upload commands of FTPS are in order and the password is not in the log. | implemented | done | simulator | `crates/p3dp-link/src/ftps::the_steps_of_an_upload_are_in_order_and_the_password_is_not_in_the_log` | - |
| P3DP-071 | Drive a real Moonraker, OctoPrint or Bambu printer. | planned | blocked | none | `planned:real_printer::a_real_printer_prints_a_job_and_stops_on_the_emergency_stop` | No driver has spoken to a real printer; it needs a person at a real printer with the power switch in reach. |
| P3DP-072 | A TLS connection opened by the program itself (stand-alone mode) only to the pinned certificate. | planned | blocked | none | `planned:tls::a_tls_connection_is_opened_only_to_the_pinned_certificate` | Not written on purpose: the Bambu link goes through the server, which holds the TLS; the seam where a connector plugs in, the pinned channel and a printer made in memory are written (`P3DP-109`), but a connector over a TLS library needs a crate that is not in the offline cache (`SERVER.md` lists the tests that are missing). |

## The EH3D server

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-073 | A job joins the queue and waits for a person. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::a_job_joins_the_queue_and_waits_for_a_person` | - |
| P3DP-074 | A job can be held, resumed and cancelled, and only one runs at a time. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::a_job_can_be_held_resumed_and_cancelled_and_only_one_runs_at_a_time` | - |
| P3DP-075 | A program over the limits is refused with its line and nothing is sent. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::a_program_over_the_limits_is_refused_with_its_line_and_nothing_is_sent` | - |
| P3DP-076 | The emergency stop latches until it is released. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::the_emergency_stop_latches_until_it_is_released` | - |
| P3DP-077 | A network that fails is tried again but a POST is never sent twice. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::a_network_that_fails_is_tried_again_but_a_post_is_never_sent_twice` | - |
| P3DP-078 | A person logs in once and the token never shows. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::a_person_logs_in_once_and_the_token_never_shows` | - |
| P3DP-079 | The refusals of the server are told in eight languages. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bridge_flow::the_refusal_of_the_server_is_told_in_eight_languages` | - |
| P3DP-080 | A repeated name is refused before anything is sent. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/queue_flow::a_repeated_name_is_refused_before_anything_is_sent_and_a_new_one_goes_in` | - |
| P3DP-081 | A job starts only after the question and only the one that was asked. | implemented | done | simulator | `src/device::a_job_starts_only_after_the_question_and_only_the_one_that_was_asked` | - |
| P3DP-082 | The emergency stop has no question and latches the machine. | implemented | done | simulator | `src/device::the_emergency_stop_has_no_question_and_latches_the_machine` | - |
| P3DP-083 | Talk to a running EH3D server with a real printer registered on it. | planned | blocked | none | `planned:real_server::a_job_reaches_a_real_printer_through_a_running_server` | The client has only met a double of the routes in memory and has never talked to a running `eh3d-server`. |

## The window, the languages and the README

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-084 | A model is loaded, reported and kept when the next one fails. | implemented | done | host-test | `src/app_state::a_model_is_loaded_reported_and_kept_when_the_next_one_fails` | - |
| P3DP-085 | Parts are arranged in rows inside the bed with the gap. | implemented | done | host-test | `src/objects::parts_are_arranged_in_rows_inside_the_bed_with_the_gap` | - |
| P3DP-086 | Overlapping parts and parts over the edge are found. | implemented | done | host-test | `src/objects::overlapping_parts_and_parts_over_the_edge_are_found` | - |
| P3DP-087 | A click picks the object under the pointer and the nearest of two. | implemented | done | host-test | `crates/p3dp-viewer/src/scene::a_click_picks_the_object_under_the_pointer_and_the_nearest_of_two` | - |
| P3DP-088 | The camera never turns past the poles and fitting a box puts all its corners in view. | implemented | done | host-test | `crates/p3dp-viewer/src/camera::fitting_a_box_puts_all_its_corners_in_view` | - |
| P3DP-089 | A slice gives a plan, the slider on the top and the summary in every language. | implemented | done | host-test | `src/preview::slicing_gives_a_plan_the_slider_on_the_top_and_the_summary_in_every_language` | - |
| P3DP-090 | A slice that fails keeps the plan it had and says why. | implemented | done | host-test | `src/preview::a_slice_that_fails_keeps_the_plan_it_had_and_says_why` | - |
| P3DP-091 | Random cameras and parts never panic the shaded view. | implemented | done | host-test | `src/shade::random_cameras_and_parts_never_panic` | - |
| P3DP-092 | The arrangement of the panels is written once when it changes and never when it does not. | implemented | done | host-test | `src/panels::the_arrangement_is_written_once_when_it_changes_and_never_when_it_does_not` | - |
| P3DP-093 | The emergency stop is never a key alone in any scheme of shortcuts. | implemented | done | host-test | `src/keys::the_emergency_stop_is_never_a_key_alone_in_any_scheme` | - |
| P3DP-094 | A change of a limit is kept in the settings, can be undone, and is the person's. | implemented | done | host-test | `src/limits_ui::a_change_is_the_persons_is_kept_in_the_settings_and_can_be_undone` | - |
| P3DP-095 | The text can be read on every background in both looks. | implemented | done | host-test | `src/look::the_text_can_be_read_on_every_background_it_is_put_on` | - |
| P3DP-096 | Every message has all eight texts and the Portuguese is that of Portugal. | implemented | done | host-test | `src/i18n::every_message_has_all_eight_texts` | - |
| P3DP-097 | The README states the real number of tests and shows only commands the program has. | implemented | done | host-test | `tests/readme::every_readme_states_the_real_number_of_tests` | - |
| P3DP-098 | The settings are read, checked and written back, and the token is hidden when written for a screen. | implemented | done | host-test | `src/config::the_token_is_hidden_when_written_for_a_screen` | - |
| P3DP-099 | A person looks at the window on a screen and uses it at a printer. | planned | blocked | none | `planned:operator::an_operator_prepares_and_starts_a_print_with_the_window` | Needs a person at a screen, at a printer. |

## Several materials and the Bambu link through the server

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-100 | A Bambu printer in LAN mode is made on the server with its certificate pinned, and its three troubles come back as stable codes in eight languages. | implemented | done | simulator | `crates/p3dp-server-bridge/tests/bambu_flow::a_certificate_that_is_not_the_pinned_one_is_untrusted_and_nothing_is_queued` | Tried only against the server in memory; nothing has run on a Bambu printer. |
| P3DP-101 | The multi-material system of a printer gives the number of tools the slicer may be asked for, and a kit that may not be fitted needs the word of the owner. | implemented | done | host-test | `crates/p3dp-profiles/src/multi::every_row_of_the_catalog_with_words_gives_two_to_sixteen_tools` | - |
| P3DP-102 | Tree supports and interface layers. | planned | pending | none | `planned:quality::a_tree_support_holds_an_overhang_and_leaves_the_part` | Not written: only plain supports and the usual quality settings exist. |
| P3DP-103 | A colour painted on a surface, the offsets of the heads of a toolchanger and the grouping of filaments by nozzle. | planned | pending | none | `planned:multi_material::a_painted_colour_changes_the_tool_inside_a_layer` | Not written: a tool changes between layers only. |

## Layers that follow the shape, the foot, the flow and the stand-alone Bambu link

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-104 | The height of the layers can follow the lean of the walls: thin where a dome lies flat, thick where a wall is straight. | implemented | done | host-test | `crates/p3dp-slicer/tests/shape::adaptive_layers_follow_the_lean_of_a_dome_and_still_make_a_good_print` | A face that is exactly flat is not followed, so a flat top may fall between two layers; no print has been made from such a stack. |
| P3DP-105 | The first layer can be drawn in from the outline against the flared foot of a print. | implemented | done | host-test | `crates/p3dp-slicer/tests/shape::the_first_layer_is_drawn_in_by_the_foot_and_the_others_are_not` | The amount that a printer needs is found on that printer; none has been tried. |
| P3DP-106 | A limit of the flow slows down every line that would ask for more plastic than the nozzle melts, and thins none. | implemented | done | host-test | `crates/p3dp-slicer/tests/shape::no_line_asks_for_more_plastic_than_the_nozzle_melts` | The limit is a figure of the settings, not measured on a hot end. |
| P3DP-107 | The flow that the maker gives for a filament holds the slice to it. | implemented | done | host-test | `src/preflight::the_slice_can_be_held_to_the_flow_of_the_filament_and_then_it_asks_no_more_than_it_takes` | The figures of the makers are those of the database of the suite and are not verified. |
| P3DP-108 | The time of a print is told by group of lines, the shares add up to the estimate and the report says them in eight languages. | implemented | done | host-test | `crates/p3dp-slicer/tests/shape::the_time_of_a_print_is_told_by_group_and_adds_up_to_the_estimate` | The estimate has the acceleration of the settings, not the planner of the printer. |
| P3DP-109 | A secure channel is handed out only after its certificate was compared with the pinned fingerprint, and a handshake that fails leaves no channel open. | prototype | partial | simulator | `crates/p3dp-link/src/tls::only_the_pinned_certificate_gets_a_channel_and_no_byte_reaches_another_printer` | Behind two traits and a printer made in memory; no connector over a TLS library exists and no TLS connection has been opened. |
| P3DP-110 | The stand-alone Bambu driver reads the state of a printer over the pinned channel into the normalized state. | prototype | partial | simulator | `crates/p3dp-link/src/bambu_lan::an_idle_printer_is_read_into_the_normalized_state` | Against a printer made in memory; nothing has been read from a real printer by this program. |
| P3DP-111 | The stand-alone Bambu driver pauses, resumes and stops a print and refuses everything else without writing a byte. | prototype | partial | simulator | `crates/p3dp-link/src/bambu_lan::pause_resume_and_stop_reach_the_printer_and_nothing_else_does` | Against a printer made in memory; the upload and the start of a file are not built, and nothing has been sent to a real printer. |
| P3DP-112 | The report of a Bambu printer is read as the figures it gives, and a damaged or partial one is refused or left out and never panics. | implemented | done | host-test | `crates/p3dp-link/src/bambu_lan::reports_with_pieces_missing_or_damaged_are_read_or_refused_and_never_panic` | The fields follow the public description of the messages of the family; none was read from a real printer by this program. |

## Printers of the suite and the import of your own files

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-113 | The catalog of printers of the suite is laid over the table of the program: its models are added, its figures win and every difference is kept. | implemented | done | host-test | `crates/p3dp-profiles/src/machines::the_suite_adds_its_models_and_keeps_the_table` | - |
| P3DP-114 | A printer of the suite is shown with everything its maker publishes, its pages, and the filaments and processes to start from, in the eight languages. | implemented | done | host-test | `src/machine_view::the_record_is_shown_with_its_pages_in_every_language` | - |
| P3DP-115 | Every starting process of every printer of the suite is a settings file that the slicer accepts. | implemented | done | host-test | `src/machine_view::every_starting_process_of_every_printer_is_accepted_by_the_slicer` | - |
| P3DP-116 | Every setting of a 3MF project, a JSON profile or an INI profile is read, and what is not read is listed with the reason. | implemented | done | host-test | `crates/p3dp-formats/src/settings_import::a_project_reads_its_settings_and_lists_everything_else` | - |
| P3DP-117 | An import tells where every key ended (applied, not applied, not used, not read), changes no value and never prints the value of a key it does not use. | implemented | done | host-test | `src/import::a_json_profile_is_brought_and_every_key_ends_somewhere` | - |
| P3DP-118 | A hostile settings file (too large, too many keys, deep nesting, numbers that are not finite, names that climb out) is refused or told and never panics. | implemented | done | host-test | `crates/p3dp-formats/src/settings_import::hostile_text_is_an_error_and_never_a_panic` | - |
| P3DP-119 | A job is checked against the printer without slicing it: its size with what is laid round it, also turned by a right angle, the height used by default, the speeds, the nozzle and the filament, in the eight languages. | implemented | done | host-test | `src/cli::fit_tells_whether_a_job_keeps_to_the_machine_in_the_language_asked` | - |
| P3DP-120 | A speed over the highest the maker states refuses the slice and names every setting; nothing is cut down. | implemented | done | host-test | `crates/p3dp-slicer/src/machine_check::a_speed_over_the_machine_refuses_and_names_every_one` | - |
| P3DP-121 | The acceleration of the estimate and the flow are held to the figures of the maker, and the program says so. | implemented | done | host-test | `crates/p3dp-slicer/src/machine_check::the_acceleration_and_the_flow_are_held_and_told_once` | - |
| P3DP-122 | A starting process and the start and end G-code of the person are applied to a slice for a Bambu, a Marlin and a Klipper printer. | implemented | done | host-test | `src/cli::slice_takes_a_process_and_your_own_start_and_end_or_refuses_them_whole` | - |
| P3DP-123 | A start or end block with a command that is not known, a temperature over the limit or a move outside the volume is refused whole. | implemented | done | host-test | `crates/p3dp-gcode/src/template::a_line_that_leaves_the_machine_or_is_not_known_refuses_the_whole_block` | - |
| P3DP-124 | An end block never homes Z nor brings the head below the height of the print. | implemented | done | host-test | `crates/p3dp-gcode/src/template::the_end_block_never_brings_the_head_down_into_the_print` | - |
| P3DP-125 | The highest speed, acceleration and flow and the default height come from the catalog of the suite, and a printer it does not have has none. | implemented | done | host-test | `crates/p3dp-profiles/src/machines::the_figures_of_motion_come_from_the_suite_and_an_unknown_printer_has_none` | - |

## Printer packages and the slice of the window

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-126 | A sliced plan of a printer of the Bambu family is written as a `.gcode.3mf` package with the G-code, its checksum, a picture and the figures of the print. | implemented | done | host-test | `src/package::a_sliced_cube_goes_into_a_package_with_its_picture_and_its_figures` | No printer has been given one; the maker does not publish the whole layout of these files. |
| P3DP-127 | The G-code of a package is read back, and a checksum that differs or is missing is told. | implemented | done | host-test | `crates/p3dp-formats/src/gcode3mf::a_checksum_that_differs_or_is_missing_is_told` | - |
| P3DP-128 | A package for a printer that does not take one, and a `.3mf` name that is not a package, are refused. | implemented | done | host-test | `src/package::each_name_writes_its_kind_and_the_wrong_ones_are_refused` | - |
| P3DP-129 | A part can be turned, sized and copied on the bed before it is sliced; copies that do not all fit refuse the slice. | implemented | done | host-test | `src/cli::slice_turns_scales_copies_and_saves_the_settings_it_used` | The copies are laid in rows; no print has been made. |
| P3DP-130 | The start and end G-code are checked without slicing and the lines about the filament follow the language. | implemented | done | host-test | `src/cli::fit_checks_your_blocks_without_slicing_and_the_filament_lines_follow_the_language` | - |
| P3DP-131 | What a slice holds to the printer is named with its figure and who states it, and a speed over the printer is refused. | implemented | done | host-test | `src/held::what_is_held_names_its_figure_and_an_unknown_printer_says_it_is_not_checked` | The panel of the window that shows it has not been looked at on a screen. |
| P3DP-132 | The window keeps the G-code as it was sliced, with the blocks of the person, and writes it or its package. | implemented | done | host-test | `src/preview::a_slice_keeps_what_was_held_the_gcode_and_gives_the_package_of_a_bambu_printer` | Tried without a screen. |
| P3DP-133 | A change of the printer, the filament, a setting or a block marks the plan as out of date. | implemented | done | host-test | `src/held::a_change_of_the_printer_the_filament_a_setting_or_a_block_makes_another_recipe` | - |
| P3DP-134 | A starting process is put over the settings whole or not at all. | implemented | done | host-test | `src/machine_view::a_starting_process_is_put_over_the_settings_whole_or_not_at_all` | - |
| P3DP-135 | The start and end G-code of the person are read from their files, and a file that cannot be read is named. | implemented | done | host-test | `src/held::the_blocks_of_the_person_are_read_or_the_file_that_cannot_be_read_is_named` | - |

## The project of the suite and its attachments

| ID | Requirement | State | Progress | Evidence | Test | Limits |
|---|---|---|---|---|---|---|
| P3DP-136 | The branch `3dp/` keeps the parts with their placement, the settings, the printer and the filament, comes back as it was, is not written again when nothing changed and leaves every other branch as it is. | implemented | done | host-test | `eh3dfile::what_is_saved_comes_back_and_saving_it_again_writes_nothing` | - |
| P3DP-137 | The G-code of a work whose parts, settings, printer or filament changed leaves the project. | implemented | done | host-test | `eh3dfile::a_work_that_changed_loses_its_old_g_code_and_keeps_the_rest` | - |
| P3DP-138 | A model or a G-code program of another program is imported with the original kept and a report, and a file that cannot be read writes nothing. | implemented | done | host-test | `eh3dfile::an_imported_model_is_kept_with_a_report_and_is_the_part_of_the_bed` | - |
| P3DP-139 | A model taken from another branch is a part of the bed with a reference that tells when its source changes, until the part is gone. | implemented | done | host-test | `eh3dfile::a_model_taken_from_cad_keeps_its_reference_until_the_part_is_gone` | - |
| P3DP-140 | The command `project` saves, lists, opens and extracts, refuses a damaged file and puts nothing but the keys of the slicer from a settings file in a project. | implemented | done | host-test | `eh3dfile::the_command_saves_lists_opens_and_extracts_and_a_damaged_file_is_an_error` | - |
| P3DP-141 | The window asks before a write that removes a signature or touches a project it did not open, and does not throw away what the project does not have. | implemented | done | host-test | `src/gui/suite::a_signed_branch_is_asked_about_and_escape_says_no` | - |
| P3DP-142 | The limits of the attachments are read from the settings file, checked and written back. | implemented | done | host-test | `src/config::the_limits_of_the_attachments_are_read_checked_and_written_back` | - |
