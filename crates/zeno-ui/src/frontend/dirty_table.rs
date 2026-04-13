use std::ops::{BitAnd, BitOr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirtyBits(u8);

impl DirtyBits {
    pub const NONE: Self = Self(0);
    pub const STYLE: Self = Self(1 << 0);
    pub const INTRINSIC: Self = Self(1 << 1);
    pub const LAYOUT: Self = Self(1 << 2);
    pub const PAINT: Self = Self(1 << 3);
    pub const SCENE: Self = Self(1 << 4);
    pub const RESOURCE: Self = Self(1 << 5);

    #[allow(dead_code)]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl BitOr for DirtyBits {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitAnd for DirtyBits {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DirtyTable {
    bits: Vec<DirtyBits>,
    generation: u64,
}

impl DirtyTable {
    pub fn new(len: usize) -> Self {
        Self { bits: vec![DirtyBits::NONE; len], generation: 0 }
    }
    pub fn mark(&mut self, index: usize, bits: DirtyBits) {
        self.bits[index] = self.bits[index] | bits;
    }
    #[allow(dead_code)]
    pub fn clear(&mut self, index: usize) {
        self.bits[index] = DirtyBits::NONE;
    }
    #[allow(dead_code)]
    pub fn clear_all(&mut self) {
        for b in &mut self.bits { *b = DirtyBits::NONE; }
    }
    #[allow(dead_code)]
    pub fn is_dirty(&self, index: usize) -> bool {
        self.bits[index] != DirtyBits::NONE
    }
    pub fn dirty_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.bits.iter().enumerate().filter_map(|(i, b)| (*b != DirtyBits::NONE).then_some(i))
    }
    pub fn bump_generation(&mut self) { self.generation += 1; }
    #[allow(dead_code)]
    pub const fn generation(&self) -> u64 { self.generation }
}
