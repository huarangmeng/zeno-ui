use crate::damage::DamageRegion;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositorFrameStats {
    pub patch_upserts: usize,
    pub patch_removes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorFrame<T> {
    pub payload: T,
    pub damage: DamageRegion,
    pub generation: u64,
    pub stats: CompositorFrameStats,
}

impl<T> CompositorFrame<T> {
    #[must_use]
    pub const fn new(
        payload: T,
        damage: DamageRegion,
        generation: u64,
        stats: CompositorFrameStats,
    ) -> Self {
        Self {
            payload,
            damage,
            generation,
            stats,
        }
    }

    #[must_use]
    pub fn full(payload: T, generation: u64) -> Self {
        Self::new(
            payload,
            DamageRegion::Full,
            generation,
            CompositorFrameStats::default(),
        )
    }
}
