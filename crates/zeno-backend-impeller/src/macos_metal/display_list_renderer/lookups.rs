use zeno_core::{Rect, Transform2D};
use zeno_scene::{DisplayList, StackingContextId};

pub(super) struct RenderLookupTables {
    spatial_world_transforms_by_id: Vec<Transform2D>,
    clip_chain_indices_by_id: Vec<Option<usize>>,
    context_indices_by_id: Vec<Option<usize>>,
    context_bounds_by_id: Vec<Option<Rect>>,
}

impl RenderLookupTables {
    pub(super) fn build(display_list: &DisplayList) -> Self {
        let spatial_len = display_list
            .spatial_tree
            .nodes
            .iter()
            .map(|node| node.id.0 as usize + 1)
            .max()
            .unwrap_or(0);
        let mut spatial_world_transforms_by_id = vec![Transform2D::identity(); spatial_len];
        for node in &display_list.spatial_tree.nodes {
            spatial_world_transforms_by_id[node.id.0 as usize] = node.world_transform;
        }

        let clip_chain_len = display_list
            .clip_chains
            .chains
            .iter()
            .map(|chain| chain.id.0 as usize + 1)
            .max()
            .unwrap_or(0);
        let mut clip_chain_indices_by_id = vec![None; clip_chain_len];
        for (index, chain) in display_list.clip_chains.chains.iter().enumerate() {
            clip_chain_indices_by_id[chain.id.0 as usize] = Some(index);
        }

        let context_len = display_list
            .stacking_contexts
            .iter()
            .map(|context| context.id.0 as usize + 1)
            .max()
            .unwrap_or(0);
        let mut context_indices_by_id = vec![None; context_len];
        let mut context_bounds_by_id = vec![Option::<Rect>::None; context_len];
        for (index, context) in display_list.stacking_contexts.iter().enumerate() {
            context_indices_by_id[context.id.0 as usize] = Some(index);
        }

        for item in &display_list.items {
            let mut current = item.stacking_context;
            while let Some(context_id) = current {
                let Some(context_index) = context_indices_by_id
                    .get(context_id.0 as usize)
                    .copied()
                    .flatten()
                else {
                    break;
                };
                let bounds = &mut context_bounds_by_id[context_id.0 as usize];
                *bounds = Some(match *bounds {
                    Some(current_bounds) => current_bounds.union(&item.visual_rect),
                    None => item.visual_rect,
                });
                current = display_list.stacking_contexts[context_index].parent;
            }
        }

        Self {
            spatial_world_transforms_by_id,
            clip_chain_indices_by_id,
            context_indices_by_id,
            context_bounds_by_id,
        }
    }

    pub(super) fn clip_chain<'a>(
        &'a self,
        display_list: &'a DisplayList,
        clip_chain_id: zeno_scene::ClipChainId,
    ) -> Option<&'a zeno_scene::ClipChain> {
        let index = self
            .clip_chain_indices_by_id
            .get(clip_chain_id.0 as usize)
            .copied()
            .flatten()?;
        display_list.clip_chains.chains.get(index)
    }

    pub(super) fn context_index(&self, context_id: StackingContextId) -> Option<usize> {
        self.context_indices_by_id
            .get(context_id.0 as usize)
            .copied()
            .flatten()
    }

    pub(super) fn context_bounds(&self, context_id: StackingContextId) -> Rect {
        self.context_bounds_by_id
            .get(context_id.0 as usize)
            .copied()
            .flatten()
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
    }

    pub(super) fn world_transform(&self, spatial_id: zeno_scene::SpatialNodeId) -> Transform2D {
        self.spatial_world_transforms_by_id
            .get(spatial_id.0 as usize)
            .copied()
            .unwrap_or_else(Transform2D::identity)
    }
}
