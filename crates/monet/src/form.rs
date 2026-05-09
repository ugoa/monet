#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Form<T>(pub T);

impl<T> std::ops::Deref for Form<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> std::ops::DerefMut for Form<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
