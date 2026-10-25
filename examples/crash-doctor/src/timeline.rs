// SPDX-License-Identifier: MIT
//! Bounded log evidence, never a profiler or proof that the game became playable.
use super::Table;
#[path = "timeline_clock.rs"]
mod clock;
const MAX_EVENTS: usize = 96;
const MAX_GAPS: usize = 32;
const MAX_CLUSTERS: usize = 32;
const GAP_MS: u64 = 5000;
#[derive(Clone)]
struct Point {
    ms: u64,
    line: usize,
    clock: String,
    segment: usize,
}
struct Cluster {
    first: Point,
    last: Point,
    count: usize,
}
fn stage(message: &str) -> Option<&'static str> {
    if message.starts_with("Loading Minecraft ") && message.contains(" with Fabric Loader ") {
        Some("Fabric loader")
    } else if message
        .strip_prefix("Loading ")
        .and_then(|value| value.strip_suffix(" mods:"))
        .is_some_and(|value| value.parse::<u32>().is_ok())
    {
        Some("Mod list")
    } else if message.starts_with("SpongePowered MIXIN Subsystem Version=") {
        Some("Mixin initialization")
    } else if message.starts_with("Backend library: LWJGL version ") {
        Some("Graphics backend")
    } else if message.starts_with("Reloading ResourceManager:") {
        Some("Resource reload")
    } else if message == "OpenAL initialized." || message == "OpenAL initialized" {
        Some("Audio backend")
    } else if message == "Sound engine started" {
        Some("Sound engine")
    } else {
        None
    }
}
fn table(columns: &[&str], rows: Vec<Vec<String>>) -> Table {
    Table {
        columns: columns.iter().map(|value| (*value).into()).collect(),
        rows,
    }
}
pub(super) fn tables(log: &str) -> Vec<Table> {
    let mut events = vec![];
    let mut gaps: Vec<(u64, Point, Point)> = vec![];
    let mut clusters: Vec<Cluster> = vec![];
    let mut cluster: Option<Cluster> = None;
    let mut previous: Option<Point> = None;
    let mut first_ms = 0;
    let mut segment = 1;
    let (
        mut timestamped,
        mut untimed,
        mut stage_count,
        mut warnings,
        mut untimed_warnings,
        mut regressions,
        mut rollovers,
        mut gap_count,
        mut cluster_count,
    ) = (
        0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize,
    );
    let finish_cluster = |value: Option<Cluster>,
                          clusters: &mut Vec<Cluster>,
                          count: &mut usize| {
        if let Some(value) = value
            && value.count >= 3
        {
            *count += 1;
            clusters.push(value);
            clusters.sort_by(|a, b| b.count.cmp(&a.count).then(a.first.line.cmp(&b.first.line)));
            clusters.truncate(MAX_CLUSTERS);
        }
    };
    for (index, line) in log.lines().enumerate() {
        let number = index + 1;
        let parsed = clock::clock(line);
        let message = clock::message(line);
        let mut elapsed = None;
        if let Some((ms, label)) = parsed {
            timestamped += 1;
            if let Some(last) = &previous {
                if ms < last.ms {
                    regressions += 1;
                    rollovers += usize::from(last.ms >= 23 * 3600 * 1000 && ms <= 3600 * 1000);
                    segment += 1;
                    first_ms = ms;
                    finish_cluster(cluster.take(), &mut clusters, &mut cluster_count);
                }
            } else {
                first_ms = ms;
            }
            let point = Point {
                ms,
                line: number,
                clock: label.into(),
                segment,
            };
            elapsed = Some(ms - first_ms);
            if let Some(last) = &previous
                && last.segment == segment
                && ms - last.ms >= GAP_MS
            {
                gap_count += 1;
                gaps.push((ms - last.ms, last.clone(), point.clone()));
                gaps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.line.cmp(&b.1.line)));
                gaps.truncate(MAX_GAPS);
            }
            if let Some(active) = &cluster
                && (ms - active.first.ms > 10_000 || active.first.segment != segment)
            {
                finish_cluster(cluster.take(), &mut clusters, &mut cluster_count);
            }
            if message.is_some_and(|(_, warning)| warning) {
                warnings += 1;
                if let Some(active) = &mut cluster {
                    active.last = point.clone();
                    active.count += 1;
                } else {
                    cluster = Some(Cluster {
                        first: point.clone(),
                        last: point.clone(),
                        count: 1,
                    });
                }
            }
            previous = Some(point);
        } else {
            untimed += 1;
            if message.is_some_and(|(_, warning)| warning) {
                warnings += 1;
                untimed_warnings += 1;
            }
        }
        if let Some((message, _)) = message
            && let Some(stage) = stage(message)
        {
            stage_count += 1;
            if events.len() < MAX_EVENTS {
                events.push(vec![
                    stage.into(),
                    parsed.map_or("", |(_, clock)| clock).into(),
                    elapsed.map_or(String::new(), |ms| ms.to_string()),
                    number.to_string(),
                    message.chars().take(180).collect(),
                    if parsed.is_some() {
                        segment.to_string()
                    } else {
                        String::new()
                    },
                ]);
            }
        }
    }
    finish_cluster(cluster, &mut clusters, &mut cluster_count);
    let summary = [
        ("Timestamped lines", timestamped.to_string()),
        ("Untimed lines", untimed.to_string()),
        (
            "Observed span ms",
            if segment == 1 {
                previous
                    .as_ref()
                    .map_or(String::new(), |last| (last.ms - first_ms).to_string())
            } else {
                String::new()
            },
        ),
        (
            "Segments",
            if timestamped == 0 {
                "0".into()
            } else {
                segment.to_string()
            },
        ),
        ("Clock regressions", regressions.to_string()),
        ("Possible midnight rollovers", rollovers.to_string()),
        ("Stage events", stage_count.to_string()),
        (
            "Unshown stage events",
            stage_count.saturating_sub(events.len()).to_string(),
        ),
        ("Warnings", warnings.to_string()),
        ("Untimed warnings", untimed_warnings.to_string()),
        (
            "Unshown gaps",
            gap_count.saturating_sub(gaps.len()).to_string(),
        ),
        (
            "Unshown clusters",
            cluster_count.saturating_sub(clusters.len()).to_string(),
        ),
    ]
    .into_iter()
    .map(|(key, value)| vec![key.into(), value])
    .collect();
    vec![
        table(&["Timeline field", "Value"], summary),
        table(
            &["Stage", "Clock", "Elapsed ms", "Line", "Excerpt", "Segment"],
            events,
        ),
        table(
            &[
                "Gap ms",
                "From line",
                "To line",
                "From clock",
                "To clock",
                "Segment",
            ],
            gaps.into_iter()
                .map(|(ms, from, to)| {
                    vec![
                        ms.to_string(),
                        from.line.to_string(),
                        to.line.to_string(),
                        from.clock,
                        to.clock,
                        from.segment.to_string(),
                    ]
                })
                .collect(),
        ),
        table(
            &[
                "First clock",
                "Last clock",
                "Warnings",
                "First line",
                "Last line",
                "Segment",
            ],
            clusters
                .into_iter()
                .map(|value| {
                    vec![
                        value.first.clock,
                        value.last.clock,
                        value.count.to_string(),
                        value.first.line.to_string(),
                        value.last.line.to_string(),
                        value.first.segment.to_string(),
                    ]
                })
                .collect(),
        ),
    ]
}
#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
