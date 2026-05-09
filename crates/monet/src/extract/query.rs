#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T> std::ops::Deref for Query<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> std::ops::DerefMut for Query<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
