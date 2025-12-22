#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MethodFilter(u16);

impl MethodFilter {
    pub const CONNECT: Self = Self::from_bits(0b0_0000_0001);

    pub const DELETE: Self = Self::from_bits(0b0_0000_0010);

    pub const GET: Self = Self::from_bits(0b0_0000_0100);

    pub const HEAD: Self = Self::from_bits(0b0_0000_1000);

    pub const OPTIONS: Self = Self::from_bits(0b0_0001_0000);

    pub const PATCH: Self = Self::from_bits(0b0_0010_0000);

    pub const POST: Self = Self::from_bits(0b0_0100_0000);

    pub const PUT: Self = Self::from_bits(0b0_1000_0000);

    pub const TRACE: Self = Self::from_bits(0b1_0000_0000);

    const fn bits(self) -> u16 {
        let bits = self;
        bits.0
    }

    const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }

    /// Performs the OR operation between the [`MethodFilter`] in `self` with `other`.
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
