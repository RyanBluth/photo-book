pub trait PersistentModifiable<T> {
    type Error;
    type Modification;

    fn load() -> Result<T, Self::Error>;
    fn save(&self) -> Result<(), Self::Error>;
    fn modify(&mut self, modification: Self::Modification) -> Result<(), Self::Error>;
}

pub struct AutoPersisting<T: PersistentModifiable<T>> {
    value: Option<T>,
}

impl<T: PersistentModifiable<T>> AutoPersisting<T> {
    pub fn new() -> Self {
        Self { value: None }
    }

    pub fn read(&mut self) -> Result<&T, T::Error> {
        if self.value.is_none() {
            self.value = Some(T::load()?);
        }
        Ok(self.value.as_ref().unwrap())
    }

    pub fn modify(&mut self, modification: T::Modification) -> Result<(), T::Error> {
        if self.value.is_none() {
            self.value = Some(T::load()?);
        }
        self.value.as_mut().unwrap().modify(modification)?;
        self.value.as_ref().unwrap().save()?;
        Ok(())
    }
}
