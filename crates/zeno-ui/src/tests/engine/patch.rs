use super::super::*;

#[test]
fn compose_submit_keeps_keyed_structure_removal_on_patch_path() {
    let first = column(vec![text("Title").key("title"), text("Body").key("body")])
        .spacing(4.0)
        .key("root");
    let removed_id = text("Body").key("body").id().0;
    let second = column(vec![text("Title").key("title")])
        .spacing(4.0)
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, current } => {
            assert!(patch.layer_removes.is_empty());
            assert!(patch.object_removes.contains(&removed_id));
            assert!(
                current
                    .objects
                    .iter()
                    .all(|block| block.object_id != removed_id)
            );
            assert!(
                current
                    .objects
                    .iter()
                    .any(|block| block.object_id == text("Title").key("title").id().0)
            );
        }
        RenderSceneUpdate::Full(_) => panic!("expected patch submit"),
    }
    assert_eq!(engine.stats().layout_passes, 2);
}

#[test]
fn keyed_structure_insert_with_layer_stays_on_patch_path() {
    let first = column(vec![text("Base").key("base")])
        .spacing(4.0)
        .key("root");
    let inserted = container(text("Overlay").key("overlay-text"))
        .key("overlay")
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer();
    let inserted_layer_id = inserted.id().0;
    let inserted_text_id = text("Overlay").key("overlay-text").id().0;
    let second = column(vec![text("Base").key("base"), inserted])
        .spacing(4.0)
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, current } => {
            assert!(
                patch
                    .layer_upserts
                    .iter()
                    .any(|layer| layer.layer_id == inserted_layer_id)
            );
            assert!(
                patch
                    .object_upserts
                    .iter()
                    .any(|block| block.object_id == inserted_text_id)
            );
            assert!(
                current
                    .layer_graph
                    .iter()
                    .any(|layer| layer.layer_id == inserted_layer_id)
            );
            let text_block = current
                .objects
                .iter()
                .find(|block| block.object_id == inserted_text_id)
                .expect("inserted text block");
            assert_eq!(text_block.layer_id, inserted_layer_id);
        }
        RenderSceneUpdate::Full(_) => panic!("expected patch submit"),
    }
}

#[test]
fn keyed_structure_edit_stays_scoped_to_smallest_container_root() {
    let first = row(vec![
        column(vec![text("A").key("a"), text("B").key("b")])
            .key("left")
            .spacing(4.0)
            .width(80.0),
        column(vec![text("Stable").key("stable")])
            .key("right")
            .spacing(4.0),
    ])
    .spacing(12.0)
    .key("root");
    let second = row(vec![
        column(vec![
            text("A").key("a"),
            text("C").key("inserted"),
            text("B").key("b"),
        ])
        .key("left")
        .spacing(4.0)
        .width(80.0),
        column(vec![text("Stable").key("stable")])
            .key("right")
            .spacing(4.0),
    ])
    .spacing(12.0)
    .key("root");
    let stable_id = text("Stable").key("stable").id().0;
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, .. } => {
            let upsert_ids: Vec<u64> = patch.object_upserts.iter().map(|block| block.object_id).collect();
            assert!(!upsert_ids.contains(&stable_id), "upserts={upsert_ids:?}");
            assert!(upsert_ids.contains(&text("C").key("inserted").id().0));
        }
        RenderSceneUpdate::Full(_) => panic!("expected patch submit"),
    }
}

#[test]
fn layout_patch_skips_unaffected_leading_siblings() {
    let first_title = text("One").key("one");
    let first_title_id = first_title.id().0;
    let second_title = text("Two").key("two");
    let second_title_id = second_title.id().0;
    let third_title = text("Three").key("three");
    let third_title_id = third_title.id().0;
    let first = column(vec![first_title, second_title, third_title])
        .spacing(4.0)
        .key("root");
    let second = column(vec![
        text("One").key("one"),
        text("Two").key("two").font_size(32.0),
        text("Three").key("three"),
    ])
    .spacing(4.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, .. } => {
            let upsert_ids: Vec<u64> = patch.object_upserts.iter().map(|block| block.object_id).collect();
            assert!(!upsert_ids.contains(&first_title_id));
            assert!(upsert_ids.contains(&second_title_id));
            assert!(upsert_ids.contains(&third_title_id));
        }
        RenderSceneUpdate::Full(_) => panic!("expected patch submit"),
    }
}

#[test]
fn fixed_container_layout_change_stays_local_to_subtree() {
    let first = row(vec![
        container(text("Short").key("text"))
            .key("card")
            .padding_all(8.0)
            .width(140.0)
            .height(48.0)
            .background(Color::WHITE),
        text("Stable").key("sibling"),
    ])
    .spacing(8.0)
    .key("root");
    let second = row(vec![
        container(text("A much longer body").key("text"))
            .key("card")
            .padding_all(8.0)
            .width(140.0)
            .height(48.0)
            .background(Color::WHITE),
        text("Stable").key("sibling"),
    ])
    .spacing(8.0)
    .key("root");
    let sibling_id = text("Stable").key("sibling").id().0;
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, .. } => {
            let upsert_ids: Vec<u64> = patch.object_upserts.iter().map(|block| block.object_id).collect();
            assert!(!upsert_ids.contains(&sibling_id), "upserts={upsert_ids:?}");
        }
        RenderSceneUpdate::Full(_) => panic!("expected patch submit"),
    }
}

#[test]
fn keyed_reorder_stays_on_layout_patch_path() {
    let first = column(vec![
        text("One").key("one"),
        text("Two").key("two"),
        text("Three").key("three"),
    ])
    .spacing(4.0)
    .key("root");
    let second = column(vec![
        text("Three").key("three"),
        text("One").key("one"),
        text("Two").key("two"),
    ])
    .spacing(4.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, current } => {
            let reordered_ids: Vec<u64> = patch
                .object_reorders
                .iter()
                .map(|reorder| reorder.object_id)
                .collect();
            assert_eq!(reordered_ids.len(), 3);
            assert!(reordered_ids.contains(&text("One").key("one").id().0));
            assert!(reordered_ids.contains(&text("Two").key("two").id().0));
            assert!(reordered_ids.contains(&text("Three").key("three").id().0));
            assert!(patch.object_upserts.len() <= 3);
            for reorder in &patch.object_reorders {
                let current_block = current
                    .objects
                    .iter()
                    .find(|block| block.object_id == reorder.object_id)
                    .expect("current reordered block");
                assert_eq!(reorder.order, current_block.order);
            }
            assert!(patch.object_removes.is_empty());
        }
        RenderSceneUpdate::Full(_) => panic!("expected reorder to stay on patch submit"),
    }
}

#[test]
fn keyed_layer_reorder_uses_layer_order_patch() {
    let first = column(vec![
        container(text("One").key("one-text"))
            .key("one")
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer(),
        container(text("Two").key("two-text"))
            .key("two")
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer(),
    ])
    .spacing(4.0)
    .key("root");
    let second = column(vec![
        container(text("Two").key("two-text"))
            .key("two")
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer(),
        container(text("One").key("one-text"))
            .key("one")
            .padding_all(8.0)
            .background(Color::WHITE)
            .opacity(0.5)
            .layer(),
    ])
    .spacing(4.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, current } => {
            let reordered_ids: Vec<u64> = patch
                .layer_reorders
                .iter()
                .map(|reorder| reorder.layer_id)
                .collect();
            assert_eq!(reordered_ids.len(), 2);
            assert!(
                reordered_ids.contains(
                    &container(text("One").key("one-text"))
                        .key("one")
                        .padding_all(8.0)
                        .background(Color::WHITE)
                        .opacity(0.5)
                        .layer()
                        .id()
                        .0
                )
            );
            assert!(
                reordered_ids.contains(
                    &container(text("Two").key("two-text"))
                        .key("two")
                        .padding_all(8.0)
                        .background(Color::WHITE)
                        .opacity(0.5)
                        .layer()
                        .id()
                        .0
                )
            );
            for reorder in &patch.layer_reorders {
                let current_layer = current
                    .layer_graph
                    .iter()
                    .find(|layer| layer.layer_id == reorder.layer_id)
                    .expect("current reordered layer");
                assert_eq!(reorder.order, current_layer.order);
            }
            assert!(patch.layer_upserts.len() <= 2);
            assert!(patch.object_removes.is_empty());
        }
        RenderSceneUpdate::Full(_) => panic!("expected reorder to stay on patch submit"),
    }
}

#[test]
fn keyed_box_reorder_stays_on_order_patch_path() {
    let first = r#box(vec![
        container(text("Back").key("back-text"))
            .key("back")
            .padding_all(8.0)
            .background(Color::rgba(20, 20, 20, 255)),
        container(text("Front").key("front-text"))
            .key("front")
            .padding_all(8.0)
            .background(Color::WHITE),
    ])
    .fixed_size(120.0, 80.0)
    .key("root");
    let second = r#box(vec![
        container(text("Front").key("front-text"))
            .key("front")
            .padding_all(8.0)
            .background(Color::WHITE),
        container(text("Back").key("back-text"))
            .key("back")
            .padding_all(8.0)
            .background(Color::rgba(20, 20, 20, 255)),
    ])
    .fixed_size(120.0, 80.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let submit = snapshot_submit(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    match submit {
        RenderSceneUpdate::Delta { delta: patch, .. } => {
            let reordered_ids: Vec<u64> = patch
                .object_reorders
                .iter()
                .map(|reorder| reorder.object_id)
                .collect();
            assert!(reordered_ids.len() >= 2);
            assert!(
                reordered_ids.contains(&container(text("Back").key("back-text")).key("back").id().0)
            );
            assert!(
                reordered_ids
                    .contains(&container(text("Front").key("front-text")).key("front").id().0)
            );
            assert!(patch.object_removes.is_empty());
        }
        RenderSceneUpdate::Full(_) => panic!("expected reorder patch"),
    }
}

#[test]
fn paint_only_patch_updates_only_dirty_blocks() {
    let title = text("Title").key("title");
    let title_id = title.id().0;
    let body = text("Body").key("body");
    let body_id = body.id().0;
    let first = column(vec![title.foreground(Color::WHITE), body])
        .spacing(4.0)
        .key("root");
    let second = column(vec![
        text("Title")
            .key("title")
            .foreground(Color::rgba(255, 220, 120, 255)),
        text("Body").key("body"),
    ])
    .spacing(4.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let (submit, display_list) =
        snapshot_outputs(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    if let RenderSceneUpdate::Delta { delta: patch, .. } = submit {
        let upsert_ids: Vec<u64> = patch.object_upserts.iter().map(|block| block.object_id).collect();
        assert_eq!(upsert_ids, vec![title_id]);
        assert!(!upsert_ids.contains(&body_id));
        assert!(patch.object_removes.is_empty());
    }
    assert_eq!(display_list.items.len(), 2);
}
