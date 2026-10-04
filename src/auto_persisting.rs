use once_cell::sync::OnceCell;

pub trait PersistentModifiable<T> {
    type Error;
    type Modification;

    fn load() -> Result<T, Self::Error>;
    fn save(&self) -> Result<(), Self::Error>;
    fn modify(&mut self, modification: Self::Modification) -> Result<(), Self::Error>;
}

pub struct AutoPersisting<T: PersistentModifiable<T>> {
    value: OnceCell<T>,
}

impl<T: PersistentModifiable<T>> AutoPersisting<T> {
    pub fn new() -> Self {
        Self {
            value: OnceCell::new(),
        }
    }

    pub fn read(&self) -> Result<&T, T::Error> {
        self.value.get_or_try_init(T::load)
    }

    pub fn modify(&mut self, modification: T::Modification) -> Result<(), T::Error> {
        self.read()?;
        let value = self.value.get_mut().unwrap();
        value.modify(modification)?;
        value.save()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestValue(u32);

    impl PersistentModifiable<TestValue> for TestValue {
        type Error = std::convert::Infallible;
        type Modification = u32;

        fn load() -> Result<Self, Self::Error> {
            Ok(Self(3))
        }

        fn save(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn modify(&mut self, value: u32) -> Result<(), Self::Error> {
            self.0 = value;
            Ok(())
        }
    }

    #[test]
    fn shared_reads_initialize_and_reuse_the_value() {
        let value = AutoPersisting::<TestValue>::new();
        assert!(value.value.get().is_none());
        assert_eq!(value.read().unwrap().0, 3);
        assert!(std::ptr::eq(value.read().unwrap(), value.read().unwrap()));
    }

    #[test]
    fn modifications_initialize_and_update_the_cached_value() {
        let mut value = AutoPersisting::<TestValue>::new();
        value.modify(7).unwrap();
        assert_eq!(value.read().unwrap().0, 7);
        value.modify(9).unwrap();
        assert_eq!(value.read().unwrap().0, 9);
    }
}
