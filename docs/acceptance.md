# Acceptance checklist

Each item names the automated test or the manual evidence that covers it.
Run `cargo test --workspace` for the automated part and
`scripts/capture_layouts.sh` for the layout captures.

| Requirement | Evidence |
| --- | --- |
| `Z = 0, ∞, ±Z0, ±jZ0` land on `(−1,0,0)`, `(1,0,0)`, `(0,0,±1)`, `(0,±1,0)` | `sphere::tests::landmark_impedances_land_on_the_documented_sphere_points` |
| `u² + v² + w² = 1` for every mapped point, including huge and tiny values | `sphere::tests::every_point_lies_on_the_unit_sphere`, `scene::tests::sphere_grid_points_are_on_the_sphere` |
| Positive and negative charts stay inside their unit discs | `sphere::tests::charts_stay_inside_their_unit_disc`, `scene::tests::grid_curves_stay_inside_the_unit_disc_and_share_geometry` |
| Shared `R = 0` boundary has identical positions in both charts | `sphere::tests::shared_boundary_has_identical_positions_in_both_charts` |
| Both charts keep inductive up, short left, open right | `sphere::tests::both_charts_keep_inductive_up_and_short_left`, `camera::tests::home_orientation_matches_the_smith_chart_axes` |
| Positive chart equals the classic Smith chart `(z − 1)/(z + 1)` | `sphere::tests::positive_chart_matches_classic_smith_chart` |
| Negative chart equals `(conj z + 1)/(conj z − 1)` | `sphere::tests::negative_chart_matches_mirrored_formula` |
| Grid circles follow the mapping | `sphere::tests::grid_circles_agree_with_the_mapping` |
| Same circuit through manual model, CSV, and Touchstone gives the same impedance | `tests/example_files.rs::series_rlc_is_identical_across_ma_ri_db_and_csv` |
| RI, MA, DB data are equivalent | `touchstone::tests::ri_ma_and_db_forms_are_equivalent`, `tests/example_files.rs` |
| Two-port order `S11 S21 S12 S22`, wrapped rows, noise block skipped | `touchstone::tests::two_port_uses_s11_s21_s12_s22_order_and_skips_noise_block`, `tests/example_files.rs::two_port_file_exposes_both_ports_and_skips_noise_data` |
| Changing the plotting `Z0` keeps physical impedance | `scene::tests::changing_the_plot_reference_keeps_physical_impedance`, `app::tests::changing_plot_z0_keeps_physical_impedance` |
| Pole `Z = −Z0` reported as divergent, `Γ = 0` phase undefined | `impedance::tests::reflection_diverges_at_negative_reference`, `format::tests::reflection_special_cases_are_spelled_out` |
| Missing phase, missing unit, unknown header, illegal option, Touchstone 2.0, 3-port rejected with reasons | `csv::tests::*`, `touchstone::tests::rejects_*`, `tests/example_files.rs::defective_inputs_are_rejected_with_reasons`, `app::tests::csv_without_unit_opens_the_import_dialog`, `app::tests::unsupported_touchstone_reports_an_error_instead_of_plotting` |
| Trajectories split at `R = 0` with an interpolated crossing; gaps never bridged | `scene::tests::crossing_traces_are_split_with_an_interpolated_boundary_vertex`, `scene::tests::gaps_break_every_view`, `scene::tests::boundary_samples_belong_to_both_charts_without_interpolation` |
| Slider and arrow keys snap to samples and skip gaps | `app::tests::arrow_stepping_skips_gaps_and_stays_in_range` |
| Single-point data shows no frequency slider | `app::tests::landmarks_show_all_traces_and_offer_no_slider` |
| `.s2p` offers `S11`/`S22` | `app::tests::touchstone_two_port_can_switch_between_ports` |
| Desktop and narrow layouts render | `app::tests::workspace_renders_at_desktop_and_narrow_sizes`, captures from `scripts/capture_layouts.sh` |
| Bundled font covers every interface character | `fonts::tests::bundled_font_covers_every_non_ascii_character_in_the_interface` |
| Three-view selection linkage | Captures `desktop-crossing.png`, `desktop-rlc-negative.png`, `desktop-landmarks.png` show the same selected point on the sphere, on the owning chart, the "see other chart" note on the other chart, and the details panel |
