// SPDX-License-Identifier: MIT
use super::*;
fn metric<'a>(tables: &'a [Table], key: &str) -> &'a str {
    &tables[0].rows.iter().find(|row| row[0] == key).unwrap()[1]
}
#[test]
fn synthetic_timeline_preserves_exact_clock_line_and_repeated_stage_evidence() {
    let log = include_str!("../fixtures/synthetic-timeline.log");
    let tables = tables(log);
    assert_eq!(metric(&tables, "Timestamped lines"), "11");
    assert_eq!(metric(&tables, "Untimed lines"), "1");
    assert_eq!(tables[1].rows[0][0], "Fabric loader");
    assert_eq!(tables[1].rows[0][3], "1");
    assert_eq!(
        tables[1]
            .rows
            .iter()
            .filter(|row| row[0] == "Resource reload")
            .count(),
        2
    );
    assert_eq!(tables[2].rows[0][0], "16000");
    assert_eq!(tables[2].rows[0][1], "6");
    assert_eq!(tables[2].rows[0][2], "7");
    assert_eq!(tables[3].rows[0][2], "3");
    assert_eq!(tables[3].rows[0][3], "4");
}
#[test]
fn clock_regressions_and_possible_midnight_start_segments_without_inferred_gap() {
    let tables = tables(
        "[23:59:59] [main/INFO]: Sound engine started\n[00:00:01] [main/INFO]: Sound engine started\n[00:00:00] [main/INFO]: Sound engine started",
    );
    assert_eq!(metric(&tables, "Segments"), "3");
    assert_eq!(metric(&tables, "Possible midnight rollovers"), "1");
    assert_eq!(metric(&tables, "Observed span ms"), "");
    assert!(tables[2].rows.is_empty());
    assert_eq!(tables[1].rows[1][2], "0");
}
#[test]
fn untimed_and_unknown_messages_never_become_inferred_stages_or_durations() {
    let tables = tables(
        "[main/INFO]: Sound engine started\nUnknown startup message\n[25:00:00] [main/WARN]: broken time\n[12:00:00] [main/INFO]: message mentions WARN",
    );
    assert_eq!(tables[1].rows.len(), 1);
    assert_eq!(tables[1].rows[0][1], "");
    assert_eq!(tables[1].rows[0][2], "");
    assert_eq!(metric(&tables, "Untimed warnings"), "1");
    assert_eq!(metric(&tables, "Warnings"), "1");
    assert!(super::tables("not a log")[1].rows.is_empty());
}
#[test]
fn timestamps_accept_millisecond_and_fabric_console_formats_only() {
    assert_eq!(
        clock::clock("[01:02:03.4] [main/INFO]: x").unwrap().0,
        3_723_400
    );
    assert_eq!(
        clock::clock("[01:02:03,045] [WARN] [FabricLoader/Test]: x")
            .unwrap()
            .0,
        3_723_045
    );
    assert_eq!(
        clock::message("[01:02:03] [WARN] [FabricLoader/Test]: x"),
        Some(("x", true))
    );
    for value in [
        "[+1:02:03]",
        "[01:2x:03]",
        "[01:02:03.1234]",
        "[界界:02:03]",
        "[01:02:60]",
    ] {
        assert!(clock::clock(value).is_none(), "{value}");
    }
}
#[test]
fn stage_gap_and_cluster_output_is_bounded_without_losing_totals() {
    let log = (0..500)
        .map(|index| {
            format!(
                "[01:{:02}:{:02}] [main/INFO]: Sound engine started",
                index / 6,
                index % 6 * 10
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tables = tables(&log);
    assert_eq!(tables[1].rows.len(), MAX_EVENTS);
    assert_eq!(tables[2].rows.len(), MAX_GAPS);
    assert!(
        metric(&tables, "Unshown stage events")
            .parse::<usize>()
            .unwrap()
            > 0
    );
    assert!(
        tables
            .iter()
            .flat_map(|table| &table.rows)
            .all(|row| row.iter().all(|cell| cell.len() <= 2048))
    );
}
#[test]
fn warning_clusters_keep_largest_bounded_rows_and_exact_unshown_count() {
    let mut log = String::new();
    for index in 0..50 {
        for offset in 0..3 {
            let seconds = index * 20 + offset;
            log.push_str(&format!(
                "[01:{:02}:{:02}] [main/WARN]: Synthetic warning\n",
                seconds / 60,
                seconds % 60
            ));
        }
    }
    let tables = tables(&log);
    assert_eq!(tables[3].rows.len(), MAX_CLUSTERS);
    assert_eq!(metric(&tables, "Unshown clusters"), "18");
    assert_eq!(metric(&tables, "Warnings"), "150");
}
