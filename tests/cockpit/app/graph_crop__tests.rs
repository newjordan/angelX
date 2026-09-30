use super::*;

pub(crate) fn begin(kind: ChartKind) -> GraphRequest {
    GraphRequest::Begin {
        plot: "harvest".into(),
        spec: ChartSpec {
            title: "Actual harvest".into(),
            kind,
            x_label: "Day".into(),
            y_label: "Bushels".into(),
            x_min: 0.0,
            x_max: 4.0,
            y_min: -10.0,
            y_max: 10.0,
            expected_points: 2,
        },
    }
}
fn point(generation: u64, index: u8, y: f64) -> GraphRequest {
    GraphRequest::Point {
        plot: "harvest".into(),
        generation,
        index,
        point: DataPoint {
            label: format!("day {index}"),
            x: f64::from(index) + 1.0,
            y,
        },
    }
}

#[test]
fn graph_crop_requires_real_points_and_complete_data_before_finishing() {
    let mut store = GraphStore::default();
    let started = store.apply(begin(ChartKind::Bar)).unwrap();
    let generation = started.chart.unwrap().generation;
    assert!(
        store
            .apply(GraphRequest::Finish {
                plot: "harvest".into(),
                generation
            })
            .is_err()
    );
    store.apply(point(generation, 0, 7.0)).unwrap();
    let last = store.apply(point(generation, 1, -3.0)).unwrap();
    assert_eq!(last.chart.unwrap().points[&1].y, -3.0);
    assert!(
        store
            .apply(GraphRequest::Finish {
                plot: "harvest".into(),
                generation
            })
            .unwrap()
            .chart
            .unwrap()
            .finished
    );
    assert!(store.apply(point(generation, 0, 6.0)).is_err());
}

#[test]
fn graph_crop_new_generation_clears_old_geometry_and_rejects_stale_data() {
    let mut store = GraphStore::default();
    let a = store
        .apply(begin(ChartKind::Bar))
        .unwrap()
        .chart
        .unwrap()
        .generation;
    store.apply(point(a, 0, 7.0)).unwrap();
    let b = store.apply(begin(ChartKind::Line)).unwrap().chart.unwrap();
    assert!(b.points.is_empty());
    assert_ne!(a, b.generation);
    assert!(store.apply(point(a, 1, 4.0)).is_err());
    assert!(
        store
            .apply(GraphRequest::Clear {
                plot: "harvest".into()
            })
            .unwrap()
            .chart
            .is_none()
    );
    assert!(store.apply(point(b.generation, 0, 1.0)).is_err());
}

#[test]
fn graph_crop_invalid_numbers_domains_labels_and_indices_do_not_mutate() {
    let mut store = GraphStore::default();
    let start = store.apply(begin(ChartKind::Scatter)).unwrap();
    let generation = start.chart.unwrap().generation;
    for request in [
        point(generation, 0, f64::NAN),
        point(generation, 0, f64::INFINITY),
        point(generation, 0, 11.0),
        point(generation, 2, 1.0),
        point(generation + 1, 0, 1.0),
    ] {
        assert!(store.apply(request).is_err());
    }
    let result = store.apply(point(generation, 0, 1.0)).unwrap();
    assert_eq!(result.revision, 2);
    let GraphRequest::Begin { mut spec, .. } = begin(ChartKind::Bar) else {
        unreachable!()
    };
    spec.y_min = 1.0;
    assert!(
        store
            .apply(GraphRequest::Begin {
                plot: "other".into(),
                spec: spec.clone()
            })
            .is_err()
    );
    spec.y_min = -10.0;
    spec.title = "bad\x1b".into();
    assert!(spec.validate().is_err());
    assert!(
        serde_json::from_value::<GraphRequest>(
            serde_json::json!({"op":"clear","plot":"harvest","execute":"shell"})
        )
        .is_err()
    );
}

#[test]
fn graph_crop_plot_count_is_bounded_and_existing_ids_can_be_regrown() {
    let mut store = GraphStore::default();
    for index in 0..MAX_PLOTS {
        let GraphRequest::Begin { spec, .. } = begin(ChartKind::Bar) else {
            unreachable!()
        };
        store
            .apply(GraphRequest::Begin {
                plot: format!("plot-{index}"),
                spec,
            })
            .unwrap();
    }
    assert!(store.apply(begin(ChartKind::Bar)).is_err());
    let GraphRequest::Begin { spec, .. } = begin(ChartKind::Line) else {
        unreachable!()
    };
    assert!(
        store
            .apply(GraphRequest::Begin {
                plot: "plot-0".into(),
                spec
            })
            .is_ok()
    );
}
